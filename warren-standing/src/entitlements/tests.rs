use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use data_encoding::BASE64URL_NOPAD;
use ed25519_dalek::{Signer, SigningKey};
use rand010::SeedableRng;
use rand010::rngs::StdRng;
use warren_api::transport::{HttpRequest, HttpResponse, HttpTransport, TransportError};
use warren_api::{
    AttributionTag, BlindingKey, EntitlementEnvelope, PubkeyHex, TokenEpochResponse,
    TokenIssueRequest, TokenIssueResponse, TokenIssuerDirectory, TokenIssuerKey, WarrenApiClient,
};
use warren_contract::pf_attribution::{CIPHERTEXT_LEN, NONCE_LEN, TAG_VERSION, signing_preimage};
use warren_identity::WarrenIdentity;
use warrenguard_natpmp_protocol::{MapProto, Request, Response, ResultCode};
use warrenguard_token::IssuerSecretKey;

use super::{
    CredentialSource, EntitlementMint, NowFn, RuleCredential, RuleSlots, SlotBatch, SlotSource,
    await_first_credential,
};
use crate::PortRefusal;

const EPOCH_SECS: u64 = 3600;
const QUOTA: u32 = 5;
const NOW: u64 = 100 * EPOCH_SECS + 5;

/// Observable state of the fake issuer, shared with the test body. The HTTP
/// transport is the mocked system boundary; the blind-RSA crypto is the real
/// engine code, so vended entitlements are real credentials.
struct IssuerState {
    keys: HashMap<u64, IssuerSecretKey>,
    /// Signs the attribution tag paired with every entitlement. The tag's
    /// ciphertext is filler: only warren-api can open one, and the client only
    /// checks the signature and the epoch before stocking it.
    attribution_key: SigningKey,
    refuse_issuance: AtomicBool,
    /// Answers every issue request with the issuer's ban refusal (403
    /// `{"error":"banned"}`, warren-core doc 105 §5.3).
    ban_wallet: AtomicBool,
    fail_transport: AtomicBool,
    /// Answers every issue request with the API's clock refusal (401
    /// `{"error":"clock_skew"}`), its `Date` this far ahead of the device:
    /// past the quarter of an hour the SDK may move a stamp forward by.
    clock_ahead_secs: Mutex<Option<u64>>,
    issue_calls: AtomicUsize,
    /// warren-api's issuance ledger: the batch that took each epoch. The SAME
    /// batch is served again with freshly minted tags, any other one is
    /// refused `already_issued`.
    ledger: Mutex<HashMap<u64, Vec<String>>>,
}

#[derive(Clone)]
struct FakeIssuer(Arc<IssuerState>);

impl FakeIssuer {
    fn new(epochs: &[u64]) -> Self {
        let mut rng = StdRng::seed_from_u64(9091);
        Self(Arc::new(IssuerState {
            keys: epochs
                .iter()
                .map(|&e| (e, IssuerSecretKey::generate(&mut rng).unwrap()))
                .collect(),
            attribution_key: SigningKey::from_bytes(&[0x42; 32]),
            refuse_issuance: AtomicBool::new(false),
            ban_wallet: AtomicBool::new(false),
            fail_transport: AtomicBool::new(false),
            clock_ahead_secs: Mutex::new(None),
            issue_calls: AtomicUsize::new(0),
            ledger: Mutex::new(HashMap::new()),
        }))
    }

    fn issue_calls(&self) -> usize {
        self.0.issue_calls.load(Ordering::SeqCst)
    }

    fn directory(&self) -> TokenIssuerDirectory {
        let mut keys: Vec<TokenIssuerKey> = self
            .0
            .keys
            .iter()
            .map(|(&epoch, sk)| {
                let pk = sk.public_key();
                TokenIssuerKey {
                    epoch,
                    token_key_id: pk.key_id().to_hex(),
                    spki_b64: BASE64URL_NOPAD.encode(&pk.to_spki()),
                    not_before: epoch * EPOCH_SECS,
                    not_after: (epoch + 1) * EPOCH_SECS,
                }
            })
            .collect();
        keys.sort_by_key(|k| k.epoch);
        TokenIssuerDirectory {
            issuer_name: "api.warrenbrowse.com".to_owned(),
            token_type: 2,
            epoch_secs: EPOCH_SECS,
            context_label: "warren/session-token/v1".to_owned(),
            quota_per_epoch: QUOTA,
            prefetch_epochs: 48,
            keys,
            attribution_verifying_key_hex: Some(
                PubkeyHex::try_from(
                    hex::encode(self.0.attribution_key.verifying_key().as_bytes()).as_str(),
                )
                .unwrap(),
            ),
            route_admission: None,
        }
    }

    fn tag(&self, epoch: u64, filler: u8) -> AttributionTag {
        let nonce = [filler; NONCE_LEN];
        let ciphertext = [filler; CIPHERTEXT_LEN];
        let signature = self
            .0
            .attribution_key
            .sign(&signing_preimage(epoch, &nonce, &ciphertext));
        let mut raw = vec![TAG_VERSION];
        raw.extend_from_slice(&epoch.to_be_bytes());
        raw.extend_from_slice(&nonce);
        raw.extend_from_slice(&ciphertext);
        raw.extend_from_slice(&signature.to_bytes());
        AttributionTag::from_bytes(&raw).unwrap()
    }
}

impl HttpTransport for FakeIssuer {
    async fn execute(&self, request: HttpRequest) -> Result<HttpResponse, TransportError> {
        if self.0.fail_transport.load(Ordering::SeqCst) {
            return Err(TransportError::Connect("fake transport down".to_owned()));
        }
        let ok = |body: Vec<u8>| Ok(HttpResponse::new(200, body));
        if request.url.ends_with("/v1/port-entitlements/keys") {
            return ok(serde_json::to_vec(&self.directory()).unwrap());
        }
        // A client that mints against the SESSION class gets credentials no
        // exit will accept for a port, so refuse to serve those paths here.
        assert!(
            request.url.ends_with("/v1/port-entitlements/issue"),
            "the entitlement mint must never reach {}",
            request.url
        );
        let call = self.0.issue_calls.fetch_add(1, Ordering::SeqCst);
        if self.0.ban_wallet.load(Ordering::SeqCst) {
            return Ok(HttpResponse::new(
                403,
                br#"{"error":"banned","reason_code":"other"}"#.to_vec(),
            ));
        }
        if let Some(ahead) = *self.0.clock_ahead_secs.lock().unwrap() {
            let server_now = std::time::SystemTime::now() + std::time::Duration::from_secs(ahead);
            return Ok(
                HttpResponse::new(401, br#"{"error":"clock_skew"}"#.to_vec())
                    .with_date(httpdate::fmt_http_date(server_now)),
            );
        }
        let req: TokenIssueRequest = serde_json::from_slice(&request.body).unwrap();
        let mut epochs = Vec::new();
        for e in &req.epochs {
            if self.0.refuse_issuance.load(Ordering::SeqCst) {
                epochs.push(TokenEpochResponse {
                    epoch: e.epoch,
                    issued: false,
                    blind_signatures: Vec::new(),
                    token_key_id: None,
                    reject_reason: Some("not_subscribed".to_owned()),
                    attribution_tags: Vec::new(),
                });
                continue;
            }
            let taken_by_another_batch = self
                .0
                .ledger
                .lock()
                .unwrap()
                .entry(e.epoch)
                .or_insert_with(|| e.blinded.clone())
                != &e.blinded;
            if taken_by_another_batch {
                epochs.push(TokenEpochResponse {
                    epoch: e.epoch,
                    issued: false,
                    blind_signatures: Vec::new(),
                    token_key_id: None,
                    reject_reason: Some("already_issued".to_owned()),
                    attribution_tags: Vec::new(),
                });
                continue;
            }
            let sk = self.0.keys.get(&e.epoch).expect("key for requested epoch");
            epochs.push(TokenEpochResponse {
                epoch: e.epoch,
                issued: true,
                blind_signatures: e
                    .blinded
                    .iter()
                    .map(|b| {
                        let bytes = BASE64URL_NOPAD.decode(b.as_bytes()).unwrap();
                        BASE64URL_NOPAD.encode(&sk.blind_sign(&bytes).unwrap())
                    })
                    .collect(),
                token_key_id: Some(sk.public_key().key_id().to_hex()),
                reject_reason: None,
                attribution_tags: (0..e.blinded.len())
                    .map(|i| self.tag(e.epoch, u8::try_from((call * 16 + i) % 256).unwrap()))
                    .collect(),
            });
        }
        ok(serde_json::to_vec(&TokenIssueResponse { epochs }).unwrap())
    }
}

/// The wallet every client here signs as.
const WALLET_SEED: [u8; 32] = [0x51; 32];

fn client(issuer: &FakeIssuer) -> WarrenApiClient<FakeIssuer> {
    WarrenApiClient::new(
        "https://api.example.test",
        WarrenIdentity::from_seed(&WALLET_SEED),
        issuer.clone(),
    )
}

fn key() -> BlindingKey {
    BlindingKey::port_entitlement(&WALLET_SEED)
}

/// The entitlement token inside an envelope, without its tag.
fn token_of(envelope: &[u8]) -> Vec<u8> {
    EntitlementEnvelope::parse(envelope)
        .expect("an envelope")
        .token()
        .to_vec()
}

/// A movable epoch clock: the handle steps time, the `NowFn` reads it.
fn clock(start: u64) -> (Arc<AtomicU64>, NowFn) {
    let t = Arc::new(AtomicU64::new(start));
    let read = t.clone();
    (t, Arc::new(move || read.load(Ordering::SeqCst)))
}

/// Wall-clock unix seconds, for the tests that drive the engine's real NAT-PMP
/// loop (real timers, so no paused clock).
fn wall_clock_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Bounded wait for the background refresh task to reach `cond`.
async fn wait_for(mut cond: impl FnMut() -> bool) {
    for _ in 0..2000 {
        if cond() {
            return;
        }
        tokio::task::yield_now().await;
    }
    panic!("background refresh never reached the expected state");
}

/// The epoch of the attribution tag inside an envelope.
fn envelope_epoch(envelope: &[u8]) -> u64 {
    EntitlementEnvelope::parse(envelope)
        .expect("a slot vends a well-formed envelope")
        .tag()
        .epoch()
}

fn refresh_loop_config(
    server: SocketAddr,
    protos: warrenguard_natpmp_client::ForwardProtos,
    credential: Option<CredentialSource>,
) -> warrenguard_natpmp_client::RefreshLoopConfig {
    warrenguard_natpmp_client::RefreshLoopConfig {
        server,
        protos,
        internal_port: 0,
        suggested_external_port: 0,
        lifetime_secs: 3600,
        suggestion: warrenguard_natpmp_client::SuggestionKind::Sticky,
        bind_addr: Some(std::net::IpAddr::V4(Ipv4Addr::LOCALHOST)),
        credential,
    }
}

async fn fake_gateway() -> (tokio::net::UdpSocket, SocketAddr) {
    let gateway = tokio::net::UdpSocket::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
        .await
        .expect("bind fake NAT-PMP gateway");
    let server = gateway.local_addr().expect("gateway addr");
    (gateway, server)
}

/// Drives one NAT-PMP cycle against a fake gateway and returns the first
/// datagram the engine's refresh loop actually put on the wire, so the
/// credential is observed where the exit reads it rather than where the mint
/// hands it over.
async fn first_map_request(credential: Option<CredentialSource>) -> Vec<u8> {
    let (gateway, server) = fake_gateway().await;
    let mut loop_handle = warrenguard_natpmp_client::spawn_refresh_loop_with(
        refresh_loop_config(
            server,
            warrenguard_natpmp_client::ForwardProtos::Udp,
            credential,
        ),
        tokio::sync::mpsc::unbounded_channel().0,
    );
    let mut buf = [0u8; 1024];
    let n = tokio::time::timeout(Duration::from_secs(5), gateway.recv(&mut buf))
        .await
        .expect("the refresh loop must send a map request")
        .expect("recv");
    loop_handle.cancel();
    buf[..n].to_vec()
}

#[tokio::test(start_paused = true)]
async fn the_batch_is_topped_up_on_the_ten_minute_cadence_and_not_before() {
    let issuer = FakeIssuer::new(&[100]);
    // The API is down when the wallet is first seen, so the immediate tick
    // stocks nothing and the request would go out bare.
    issuer.0.fail_transport.store(true, Ordering::SeqCst);
    let (_t, now) = clock(NOW);
    let mint = EntitlementMint::new(now);
    let rule = mint.rule_credential([1; 32], 0, key(), || client(&issuer));
    let source = rule.provider();
    tokio::task::yield_now().await;
    assert!(source().is_none(), "nothing minted yet");

    // The API heals, but the cadence is coarse on purpose: nothing is fetched
    // again before the interval elapses.
    issuer.0.fail_transport.store(false, Ordering::SeqCst);
    tokio::time::advance(Duration::from_secs(599)).await;
    tokio::task::yield_now().await;
    assert_eq!(
        issuer.issue_calls(),
        0,
        "the mint must not retry before the interval elapses"
    );

    tokio::time::advance(Duration::from_secs(1)).await;
    wait_for(|| issuer.issue_calls() >= 1).await;
    assert!(source().is_some(), "the tick must stock the batch");
}

#[tokio::test(start_paused = true)]
async fn no_refresh_reaches_the_issuer_while_the_account_is_logged_out() {
    let issuer = FakeIssuer::new(&[100]);
    let (_t, now) = clock(NOW);
    let (activity, gate) = tokio::sync::watch::channel(false);
    let mint = EntitlementMint::new(now).with_activity(gate);
    let source = mint.slot_source([1; 32], key(), || client(&issuer));

    tokio::time::advance(Duration::from_secs(3 * 600)).await;
    tokio::task::yield_now().await;
    assert_eq!(
        issuer.issue_calls(),
        0,
        "no issuance for a logged-out wallet"
    );
    assert!(source.credential(0).is_none());

    activity.send(true).unwrap();
    wait_for(|| issuer.issue_calls() >= 1).await;
    assert!(
        source.credential(0).is_some(),
        "a login stocks the batch without waiting for the next tick"
    );
}

#[tokio::test(start_paused = true)]
async fn the_issuers_ban_refusal_reaches_the_standing_of_the_wallet_it_refused() {
    let issuer = FakeIssuer::new(&[100]);
    issuer.0.ban_wallet.store(true, Ordering::SeqCst);
    let (_t, now) = clock(NOW);
    let standing = Arc::new(crate::StandingStore::new(None));
    let sink = standing.clone();
    let mint = EntitlementMint::new(now).with_ban_sink(Arc::new(move |wallet, error| {
        sink.on_refresh_error(wallet, error, NOW)
    }));
    let _source = mint.slot_source([1; 32], key(), || client(&issuer));

    wait_for(|| standing.ban_in_force(&[1; 32], NOW).is_some()).await;

    assert_eq!(
        standing.ban_in_force(&[1; 32], NOW).map(|ban| ban.reason),
        Some(warren_api::BanReasonCode::Other)
    );
    assert_eq!(
        standing.ban_in_force(&[2; 32], NOW),
        None,
        "only that wallet"
    );
}

/// Forum topic 219: every refresh was refused for the device's clock, and the
/// rules said the batch was used up. A bare request refused then names the
/// clock, and how far off it is, so the person can set it right.
#[tokio::test(start_paused = true)]
async fn a_bare_request_refused_while_the_issuer_refuses_the_clock_names_the_clock() {
    let issuer = FakeIssuer::new(&[100]);
    *issuer.0.clock_ahead_secs.lock().unwrap() = Some(3_600);
    let (_t, now) = clock(NOW);
    let mint = EntitlementMint::new(now);
    let rule = mint.rule_credential([1; 32], 0, key(), || client(&issuer));
    wait_for(|| issuer.issue_calls() >= 1).await;
    // The refusal is recorded once the refresh pass has ended.
    for _ in 0..50 {
        tokio::task::yield_now().await;
    }

    assert_eq!(rule.provider()(), None, "nothing could be minted");
    match rule.on_refused() {
        PortRefusal::ClockSkew {
            offset_secs: Some(offset),
        } => assert!(offset.abs_diff(3_600) <= 2, "{offset}"),
        other => panic!("the clock must be named, got {other:?}"),
    }
}

/// Once a refresh goes through, the clock is no longer the reason: a batch
/// that is still empty then says so without blaming the clock.
#[tokio::test(start_paused = true)]
async fn a_refresh_that_goes_through_stops_blaming_the_clock() {
    let issuer = FakeIssuer::new(&[100]);
    *issuer.0.clock_ahead_secs.lock().unwrap() = Some(3_600);
    let (_t, now) = clock(NOW);
    let mint = EntitlementMint::new(now);
    let rule = mint.rule_credential([1; 32], 0, key(), || client(&issuer));
    wait_for(|| issuer.issue_calls() >= 1).await;

    *issuer.0.clock_ahead_secs.lock().unwrap() = None;
    issuer.0.refuse_issuance.store(true, Ordering::SeqCst);
    tokio::time::advance(super::REFRESH_INTERVAL).await;
    wait_for(|| issuer.issue_calls() >= 2).await;
    for _ in 0..50 {
        tokio::task::yield_now().await;
    }

    assert_eq!(rule.provider()(), None);
    assert_eq!(rule.on_refused(), PortRefusal::NoEntitlement);
}

#[tokio::test(start_paused = true)]
async fn a_slot_keeps_one_envelope_for_the_whole_epoch_across_reconnects() {
    let issuer = FakeIssuer::new(&[100]);
    let (_t, now) = clock(NOW);
    let mint = EntitlementMint::new(now);
    let source = mint.slot_source([1; 32], key(), || client(&issuer));
    wait_for(|| issuer.issue_calls() >= 1).await;

    let first = source
        .credential(0)
        .expect("a stocked batch vends the slot");
    assert_eq!(
        first.len(),
        warren_contract::pf_attribution::ENVELOPE_LEN,
        "a slot presents the whole envelope, token and tag"
    );
    assert_eq!(
        source.credential(0),
        Some(first.clone()),
        "a renewal must re-present the envelope the exit already spent"
    );

    // A reconnect of the same wallet must not build a second manager, and the
    // same slot keeps the same envelope.
    let redial = mint.slot_source([1; 32], key(), || unreachable!("manager must be reused"));
    assert_eq!(redial.credential(0), Some(first.clone()));

    // A second slot draws its own: two live rules never read as one port.
    assert_ne!(redial.credential(1).expect("slot 1 draws its own"), first);
}

#[tokio::test(start_paused = true)]
async fn a_slot_presents_the_next_epochs_envelope_once_the_epoch_rolls_over() {
    let issuer = FakeIssuer::new(&[100, 101]);
    let (t, now) = clock(NOW);
    let mint = EntitlementMint::new(now);
    let source = mint.slot_source([1; 32], key(), || client(&issuer));
    wait_for(|| issuer.issue_calls() >= 1).await;
    let during = source.credential(0).expect("epoch 100 is stocked");

    t.store(101 * EPOCH_SECS + 5, Ordering::SeqCst);
    let after = source
        .credential(0)
        .expect("the prefetched epoch 101 is stocked");

    assert_eq!(envelope_epoch(&during), 100);
    assert_eq!(
        envelope_epoch(&after),
        101,
        "an entitlement verifies against its own epoch's key only"
    );
}

#[tokio::test(start_paused = true)]
async fn the_first_request_waits_for_a_mint_that_is_still_landing() {
    let issuer = FakeIssuer::new(&[100]);
    let (_t, now) = clock(NOW);
    let mint = EntitlementMint::new(now);
    let rule = mint.rule_credential([1; 32], 0, key(), || client(&issuer));
    let source = rule.provider();

    // The mint is cold at the instant the loop would fire its first cycle; the
    // wait is what keeps that request from going out bare.
    assert!(source().is_none());
    let credential =
        await_first_credential(&source, Duration::from_secs(8), Duration::from_millis(250)).await;
    assert_eq!(credential, source(), "the wait vends what the slot holds");
    assert!(credential.is_some(), "a landing mint must be waited for");
}

#[tokio::test(start_paused = true)]
async fn a_mint_that_never_lands_lets_the_request_go_bare() {
    let issuer = FakeIssuer::new(&[100]);
    issuer.0.fail_transport.store(true, Ordering::SeqCst);
    let (_t, now) = clock(NOW);
    let mint = EntitlementMint::new(now);
    let rule = mint.rule_credential([1; 32], 0, key(), || client(&issuer));
    let source = rule.provider();

    let started = tokio::time::Instant::now();
    let credential =
        await_first_credential(&source, Duration::from_secs(8), Duration::from_millis(250)).await;
    assert!(credential.is_none());
    assert!(
        started.elapsed() >= Duration::from_secs(8),
        "the wait must be bounded by the grace, not by the mint"
    );
}

/// The refresh must outlive the runtime of the caller that first asked for the
/// wallet's source: on iOS that is a tunnel's runtime, shut down at every
/// disconnect, while the manager lives for the whole process.
#[test]
fn the_refresh_runs_on_the_mints_runtime_not_the_callers() {
    let process = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .unwrap();
    let issuer = FakeIssuer::new(&[100]);
    let (_t, now) = clock(NOW);
    let mint = EntitlementMint::new(now).with_runtime(process.handle().clone());

    // A current-thread runtime polls nothing it spawned before `block_on`
    // yields, so a refresh spawned there dies unpolled with it.
    let tunnel = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let source = tunnel.block_on(async { mint.slot_source([1; 32], key(), || client(&issuer)) });
    drop(tunnel);

    for _ in 0..500 {
        if source.credential(0).is_some() {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("the refresh died with the runtime that asked for the source");
}

#[tokio::test]
async fn the_map_request_carries_the_slots_envelope_in_its_trailer() {
    // The issuer must publish the epoch the wall clock is in: these tests run
    // the engine's real refresh loop, so they cannot pause time.
    let issuer = FakeIssuer::new(&[wall_clock_secs() / EPOCH_SECS]);
    let mint = EntitlementMint::new(Arc::new(wall_clock_secs));
    let rule = mint.rule_credential([1; 32], 0, key(), || client(&issuer));
    let source = rule.provider();
    wait_for(|| issuer.issue_calls() >= 1).await;
    let expected = source().expect("a stocked batch vends the slot");

    let datagram = first_map_request(Some(source)).await;
    assert_eq!(
        warrenguard_natpmp_protocol::credential_trailer(&datagram),
        Some(expected.as_slice()),
        "the exit must read this rule's envelope off the request"
    );
}

/// A TCP+UDP pair is one port: the engine maps the UDP leg first, then pins
/// the TCP leg to the port it was granted, and both requests must carry the
/// same envelope, or the pair would spend two entitlements.
#[tokio::test]
async fn both_legs_of_a_pair_present_the_same_envelope() {
    let issuer = FakeIssuer::new(&[wall_clock_secs() / EPOCH_SECS]);
    let mint = EntitlementMint::new(Arc::new(wall_clock_secs));
    let rule = mint.rule_credential([1; 32], 0, key(), || client(&issuer));
    let source = rule.provider();
    wait_for(|| issuer.issue_calls() >= 1).await;
    let expected = source().expect("a stocked batch vends the slot");

    let (gateway, server) = fake_gateway().await;
    let mut loop_handle = warrenguard_natpmp_client::spawn_refresh_loop_with(
        refresh_loop_config(
            server,
            warrenguard_natpmp_client::ForwardProtos::Both,
            Some(source),
        ),
        tokio::sync::mpsc::unbounded_channel().0,
    );
    let mut legs = Vec::new();
    let mut buf = [0u8; 1024];
    while legs.len() < 2 {
        let (n, from) = tokio::time::timeout(Duration::from_secs(5), gateway.recv_from(&mut buf))
            .await
            .expect("the refresh loop must map both legs")
            .expect("recv");
        let Ok(Request::Map { proto, .. }) = warrenguard_natpmp_protocol::parse_request(&buf[..n])
        else {
            continue;
        };
        legs.push((
            proto,
            warrenguard_natpmp_protocol::credential_trailer(&buf[..n]).map(<[u8]>::to_vec),
        ));
        let grant = Response::Map {
            proto,
            result_code: ResultCode::Success,
            epoch_secs: 1,
            internal_port: 0,
            external_port: 50_000,
            lifetime_secs: 3600,
            rate_limit: None,
        };
        gateway
            .send_to(
                &warrenguard_natpmp_protocol::serialize_response(&grant),
                from,
            )
            .await
            .expect("answer the leg");
    }
    loop_handle.cancel();

    assert_eq!(
        legs,
        vec![
            (MapProto::Udp, Some(expected.clone())),
            (MapProto::Tcp, Some(expected)),
        ]
    );
}

#[tokio::test]
async fn an_issuer_with_no_entitlement_leaves_the_request_bare() {
    let issuer = FakeIssuer::new(&[wall_clock_secs() / EPOCH_SECS]);
    issuer.0.refuse_issuance.store(true, Ordering::SeqCst);
    let mint = EntitlementMint::new(Arc::new(wall_clock_secs));
    let rule = mint.rule_credential([1; 32], 0, key(), || client(&issuer));
    let source = rule.provider();
    wait_for(|| issuer.issue_calls() >= 1).await;
    assert!(
        source().is_none(),
        "an issuer that refuses must not fabricate a credential"
    );

    // The mapping still goes out, bare, and the exit refuses it: the client
    // must never borrow another slot's envelope to avoid that.
    let datagram = first_map_request(Some(source)).await;
    assert_eq!(
        datagram.len(),
        warrenguard_natpmp_protocol::MAP_REQUEST_LEN,
        "no entitlement must leave the RFC frame untouched"
    );
}

#[test]
fn two_live_rules_never_hold_the_same_slot() {
    let slots = RuleSlots::new();

    let first = slots.acquire();
    let second = slots.acquire();

    assert_eq!((first.slot(), second.slot()), (0, 1));
}

/// Freed with the rule, and reused rather than grown: a rule rebuilt in the
/// same epoch presents the envelope the exit already spent for its port, and
/// slots that only grew would run past the batch after a few rule changes.
#[test]
fn a_freed_slot_goes_to_the_next_rule() {
    let slots = RuleSlots::new();
    let first = slots.acquire();
    let second = slots.acquire();

    drop(first);

    assert_eq!(slots.acquire().slot(), 0);
    assert_eq!(second.slot(), 1);
}

/// Read on every cycle, never captured: the slot's envelope changes when the
/// epoch rolls over, and the next renewal must carry the new one.
#[test]
fn a_rule_credential_reads_its_own_slot_on_every_cycle() {
    let generation = Arc::new(AtomicU64::new(1));
    let current = generation.clone();
    let source: SlotSource = Arc::new(move |slot| {
        let slot = u8::try_from(slot).ok()?;
        let generation = u8::try_from(current.load(Ordering::SeqCst)).ok()?;
        Some(vec![slot, generation])
    });
    let rule = RuleCredential::new(source, 3);
    let provider = rule.provider();

    assert_eq!(provider(), Some(vec![3, 1]));
    generation.store(2, Ordering::SeqCst);
    assert_eq!(provider(), Some(vec![3, 2]));
}

#[test]
fn a_rule_credential_records_whether_its_last_request_carried_one() {
    let carrying = Arc::new(AtomicBool::new(false));
    let answer = carrying.clone();
    let source: SlotSource = Arc::new(move |_| answer.load(Ordering::SeqCst).then(|| vec![1, 2]));
    let rule = RuleCredential::new(source, 0);
    let provider = rule.provider();

    assert_eq!(provider(), None);
    assert!(!rule.presented());
    carrying.store(true, Ordering::SeqCst);
    assert_eq!(provider(), Some(vec![1, 2]));
    assert!(rule.presented());
}

/// A batch answering `answer` for every slot, counting what the rules report.
#[derive(Default)]
struct CountingBatch {
    answer: Option<Vec<u8>>,
    refused: AtomicUsize,
    released: AtomicUsize,
}

impl SlotBatch for CountingBatch {
    fn credential(&self, _slot: usize) -> Option<Vec<u8>> {
        self.answer.clone()
    }

    fn refused(&self, _slot: usize, _presented: &[u8]) {
        self.refused.fetch_add(1, Ordering::SeqCst);
    }

    fn hold(&self, _slot: usize) -> u64 {
        0
    }

    fn release(&self, _slot: usize, _claim: u64) {
        self.released.fetch_add(1, Ordering::SeqCst);
    }
}

/// A bare request refused says nothing about any entitlement: the slot stays
/// where it is, and the next mint or epoch brings it one.
#[test]
fn a_refused_bare_request_moves_no_slot() {
    let batch = Arc::new(CountingBatch::default());
    let rule = RuleCredential::new(batch.clone(), 0);
    let _ = rule.provider()();

    assert_eq!(
        rule.on_refused(),
        PortRefusal::NoEntitlement,
        "nothing was presented"
    );
    assert_eq!(batch.refused.load(Ordering::SeqCst), 0);
}

/// Held by a rule that is gone, the entitlement would leave a refused rule
/// only the ones further up the batch, or none once the others are held. A
/// refresh loop still winding down must not take the slot back.
#[test]
fn a_rule_gone_releases_its_slot_and_its_provider_answers_nothing() {
    let batch = Arc::new(CountingBatch {
        answer: Some(vec![1, 2]),
        ..CountingBatch::default()
    });
    let rule = RuleCredential::new(batch.clone(), 2);
    let provider = rule.provider();
    assert_eq!(provider(), Some(vec![1, 2]));

    drop(rule);

    assert_eq!(batch.released.load(Ordering::SeqCst), 1);
    assert_eq!(provider(), None);
}

/// A rule rebuilt on its slot (an iOS re-bind, a new Android session) can be
/// taken before the last reference to its predecessor is gone: a refresh
/// loop's task is dropped late, on a worker thread. That late release must
/// not take the new rule's entitlement away, or a rule that had moved off
/// another device's serial goes back to it at the next epoch and is refused.
#[tokio::test(start_paused = true)]
async fn a_rule_gone_after_its_successor_took_the_slot_leaves_the_successor_its_place() {
    let issuer = FakeIssuer::new(&[100, 101]);
    let (t, now) = clock(NOW);
    let mint = EntitlementMint::new(now.clone());
    let before = mint.rule_credential([1; 32], 0, key(), || client(&issuer));
    wait_for(|| issuer.issue_calls() >= 1).await;
    let _ = before.provider()().expect("slot 0 stocked");
    assert_eq!(
        before.on_refused(),
        PortRefusal::EntitlementRefused,
        "moved off place 0"
    );
    let after = mint.rule_credential([1; 32], 0, key(), || unreachable!("one manager"));
    let moved = token_of(&after.provider()().expect("the successor's place"));

    drop(before);

    let elsewhere = EntitlementMint::new(now);
    let place_zero = elsewhere.rule_credential([1; 32], 0, key(), || client(&issuer));
    t.store(101 * EPOCH_SECS + 5, Ordering::SeqCst);
    let next = token_of(&after.provider()().expect("epoch 101 stocked"));
    wait_for(|| place_zero.provider()().is_some()).await;
    assert_ne!(
        next,
        token_of(&place_zero.provider()().expect("stocked")),
        "the successor was sent back to place 0"
    );
    assert_ne!(
        moved, next,
        "an entitlement verifies against its epoch only"
    );
}

/// Another device of the wallet holds the same batch, and its first rule
/// presents what this one's first rule presents: the exit leases that serial
/// to one port fleet-wide. Asked again on the same serial, this rule would be
/// refused for as long as the other device's port lives.
#[tokio::test(start_paused = true)]
async fn a_refused_rule_moves_to_an_entitlement_no_other_rule_holds() {
    let issuer = FakeIssuer::new(&[100]);
    let (_t, now) = clock(NOW);
    let mint = EntitlementMint::new(now);
    let first = mint.rule_credential([1; 32], 0, key(), || client(&issuer));
    let second = mint.rule_credential([1; 32], 1, key(), || unreachable!("one manager"));
    wait_for(|| issuer.issue_calls() >= 1).await;
    let refused = token_of(&first.provider()().expect("slot 0 stocked"));
    let other = token_of(&second.provider()().expect("slot 1 stocked"));

    assert_eq!(
        first.on_refused(),
        PortRefusal::EntitlementRefused,
        "an entitlement was presented"
    );

    let moved = token_of(&first.provider()().expect("another entitlement"));
    assert_ne!(moved, refused, "the refused serial is presented again");
    assert_ne!(moved, other, "two rules now present one serial");
}

/// A daemon restart, an app update or a reboot builds a new mint for the
/// wallet. The batch is derived from the wallet, so the issuer serves it
/// again instead of `already_issued` for the rest of the 48 h horizon, and
/// the rule re-presents what the exit already spent for its port.
#[tokio::test(start_paused = true)]
async fn a_restarted_process_presents_the_entitlement_its_rule_held() {
    let issuer = FakeIssuer::new(&[100]);
    let (_t, now) = clock(NOW);
    let before = EntitlementMint::new(now.clone());
    let rule = before.rule_credential([1; 32], 0, key(), || client(&issuer));
    wait_for(|| issuer.issue_calls() >= 1).await;
    let held = token_of(&rule.provider()().expect("stocked"));
    drop((rule, before));

    let restarted = EntitlementMint::new(now);
    let rule = restarted.rule_credential([1; 32], 0, key(), || client(&issuer));
    let provider = rule.provider();
    wait_for(|| issuer.issue_calls() >= 2).await;

    wait_for(|| provider().is_some()).await;
    assert_eq!(token_of(&provider().expect("served")), held);
}
