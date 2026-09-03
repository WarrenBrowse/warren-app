//! Port-forwarding entitlements (warren-core doc 99) for the Android tunnel:
//! the warren-jni twin of the desktop daemon's
//! `mullvad-daemon::warren_port_entitlements`.
//!
//! One [`warren_api::PortEntitlementManager`] per wallet, process-lived, and a
//! background top-up on the same coarse timer the session-token mint uses, so
//! issuance timing mirrors neither the user enabling port forwarding nor the
//! session it will be spent on.
//!
//! The source is keyed by SLOT, not by rule: one entitlement buys one
//! forwarded port, so the caller owns which rule holds which slot and this
//! module only answers "what does slot n present right now". Android runs the
//! single preferred-port model, so today that is slot 0.
//!
//! An exhausted batch, an unreachable API and a wallet the issuer refuses all
//! answer `None`, and the exit then applies its configured per-client quota.
//! Degrading that way is the documented behaviour: an outage costs the
//! fleet-wide cap, never the feature.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use warren_api::{HttpTransport, PortEntitlementManager, WarrenApiClient};

/// Unix-seconds clock seam. The clock is a system boundary (shared TDD rule):
/// tests drive epochs deterministically, production wires the system clock.
type NowFn = Arc<dyn Fn() -> u64 + Send + Sync>;

/// What one NAT-PMP cycle presents, or `None` for a bare request.
///
/// Structurally the engine's `warrenguard_natpmp_client::CredentialProvider`,
/// spelled out here so the mint stays host-compiled: the engine's NAT-PMP
/// crate is an Android-only dependency.
pub(crate) type CredentialSource = Arc<dyn Fn() -> Option<Vec<u8>> + Send + Sync>;

/// How often the batch is topped up. Matches the session-token mint and the
/// desktop daemon: coarse on purpose (see the module docs).
const REFRESH_INTERVAL: Duration = Duration::from_secs(600);

/// Process-lived registry of one [`PortEntitlementManager`] per wallet.
///
/// Reused across reconnects so a rule that reconnects presents the credential
/// the exit already spent for it rather than burning a second one.
pub(crate) struct EntitlementMint<T> {
    now: NowFn,
    managers: parking_lot::Mutex<HashMap<[u8; 32], Arc<PortEntitlementManager<T>>>>,
}

impl<T: HttpTransport + 'static> EntitlementMint<T> {
    pub(crate) fn new(now: NowFn) -> Self {
        Self {
            now,
            managers: parking_lot::Mutex::new(HashMap::new()),
        }
    }

    /// The credential source rule `slot` of `wallet_pubkey` presents. First
    /// sight of a wallet builds its manager (via `make_client`, which owns the
    /// wallet identity) and starts its background refresh; later calls reuse
    /// both, so the factory runs at most once per wallet and process.
    pub(crate) fn credential_source(
        &self,
        wallet_pubkey: [u8; 32],
        slot: usize,
        make_client: impl FnOnce() -> WarrenApiClient<T>,
    ) -> CredentialSource {
        let manager = {
            let mut managers = self.managers.lock();
            managers
                .entry(wallet_pubkey)
                .or_insert_with(|| {
                    let manager = Arc::new(PortEntitlementManager::new(Arc::new(make_client())));
                    spawn_refresh(manager.clone(), self.now.clone());
                    manager
                })
                .clone()
        };
        let now = self.now.clone();
        Arc::new(move || manager.credential_for_slot(slot, now()))
    }
}

/// Background top-up: the first tick fires immediately (stock as soon as a
/// wallet is seen), then every [`REFRESH_INTERVAL`], exactly like the desktop
/// twin. The manager only mints epochs it has not attempted yet, so in steady
/// state a tick costs one unsigned directory fetch.
fn spawn_refresh<T: HttpTransport + 'static>(manager: Arc<PortEntitlementManager<T>>, now: NowFn) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(REFRESH_INTERVAL);
        loop {
            tick.tick().await;
            if let Err(e) = manager.refresh_auto(now()).await {
                // Transient: the batch keeps vending what it already holds and
                // the next tick retries. The error chain carries no credential
                // or seed material.
                log::warn!("Warren port-entitlement refresh failed (keeping existing): {e}");
            }
        }
    });
}

#[cfg(all(target_os = "android", feature = "tunnel"))]
pub(crate) use android::provider_for;

#[cfg(all(target_os = "android", feature = "tunnel"))]
mod android {
    use std::sync::{Arc, OnceLock};

    use ed25519_dalek::SigningKey;
    use warren_api::WarrenApiClient;
    use warren_identity::WarrenIdentity;

    use super::{CredentialSource, EntitlementMint};
    use crate::protected_transport::ProtectedTransport;

    /// Android runs the single preferred-port model (one rule, one forwarded
    /// port), so every session draws the first slot of the batch. The desktop
    /// multi-rule editor is what makes slot assignment dynamic there.
    const ANDROID_RULE_SLOT: usize = 0;

    /// Process-lived mint registry: survives connect/disconnect cycles so the
    /// refresh cadence stays decoupled from session timing, and so a redial
    /// re-presents the credential the exit already spent for this port. The
    /// transport is the VpnService-protected one, for the same reason as the
    /// token mint: an unprotected socket loses the tunnel bring-up race.
    static MINT: OnceLock<EntitlementMint<ProtectedTransport>> = OnceLock::new();

    fn now_unix_secs() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }

    /// The entitlement source for `signing_key`'s wallet against the compiled
    /// product API. The minting identity is built from the SAME Ed25519 key
    /// the tunnel handshake signs with, so the minting wallet is bit-for-bit
    /// the subscribed wallet.
    pub(crate) fn provider_for(signing_key: SigningKey) -> CredentialSource {
        let mint = MINT.get_or_init(|| EntitlementMint::new(Arc::new(now_unix_secs)));
        let wallet_pubkey = signing_key.verifying_key().to_bytes();
        mint.credential_source(wallet_pubkey, ANDROID_RULE_SLOT, move || {
            WarrenApiClient::new(
                crate::product::PRODUCT_API_URL.to_owned(),
                WarrenIdentity::from_signing_key(signing_key),
                ProtectedTransport::new(),
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::net::{Ipv4Addr, SocketAddr};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
    use std::time::Duration;

    use data_encoding::BASE64URL_NOPAD;
    use rand010::SeedableRng;
    use rand010::rngs::StdRng;
    use warren_api::transport::{HttpRequest, HttpResponse, HttpTransport, TransportError};
    use warren_api::{
        TokenEpochResponse, TokenIssueRequest, TokenIssueResponse, TokenIssuerDirectory,
        TokenIssuerKey, WarrenApiClient,
    };
    use warren_identity::WarrenIdentity;
    use warrenguard_token::IssuerSecretKey;

    use super::{CredentialSource, EntitlementMint, NowFn};

    const EPOCH_SECS: u64 = 3600;
    const QUOTA: u32 = 5;
    const NOW: u64 = 100 * EPOCH_SECS + 5;

    /// Observable state of the fake issuer, shared with the test body. The
    /// HTTP transport is the mocked system boundary; the blind-RSA crypto is
    /// the real engine code, so vended entitlements are real credentials.
    struct IssuerState {
        keys: HashMap<u64, IssuerSecretKey>,
        refuse_issuance: AtomicBool,
        fail_transport: AtomicBool,
        issue_calls: AtomicUsize,
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
                refuse_issuance: AtomicBool::new(false),
                fail_transport: AtomicBool::new(false),
                issue_calls: AtomicUsize::new(0),
            }))
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
            }
        }
    }

    impl HttpTransport for FakeIssuer {
        async fn execute(&self, request: HttpRequest) -> Result<HttpResponse, TransportError> {
            if self.0.fail_transport.load(Ordering::SeqCst) {
                return Err(TransportError::Connect("fake transport down".to_owned()));
            }
            let ok = |body: Vec<u8>| Ok(HttpResponse { status: 200, body });
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
            self.0.issue_calls.fetch_add(1, Ordering::SeqCst);
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
                });
            }
            ok(serde_json::to_vec(&TokenIssueResponse { epochs }).unwrap())
        }
    }

    fn client(issuer: &FakeIssuer) -> WarrenApiClient<FakeIssuer> {
        WarrenApiClient::new(
            "https://api.example.test",
            WarrenIdentity::from_seed(&[0x51; 32]),
            issuer.clone(),
        )
    }

    /// A movable epoch clock: the handle steps time, the `NowFn` reads it.
    fn clock(start: u64) -> (Arc<AtomicU64>, NowFn) {
        let t = Arc::new(AtomicU64::new(start));
        let read = t.clone();
        (t, Arc::new(move || read.load(Ordering::SeqCst)))
    }

    /// Wall-clock unix seconds, for the two tests that drive the engine's real
    /// NAT-PMP loop (real timers, so no paused clock).
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

    /// Drives one NAT-PMP cycle against a fake gateway and returns the first
    /// datagram the engine's refresh loop actually put on the wire, so the
    /// credential is observed where the exit reads it rather than where the
    /// mint hands it over.
    async fn first_map_request(credential: Option<CredentialSource>) -> Vec<u8> {
        let gateway = tokio::net::UdpSocket::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
            .await
            .expect("bind fake NAT-PMP gateway");
        let server = gateway.local_addr().expect("gateway addr");
        let mut loop_handle = warrenguard_natpmp_client::spawn_refresh_loop_with(
            warrenguard_natpmp_client::RefreshLoopConfig {
                server,
                protos: warrenguard_natpmp_client::ForwardProtos::Udp,
                internal_port: 0,
                suggested_external_port: 0,
                lifetime_secs: 3600,
                suggestion: warrenguard_natpmp_client::SuggestionKind::Sticky,
                bind_addr: Some(std::net::IpAddr::V4(Ipv4Addr::LOCALHOST)),
                credential,
            },
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
        let source = mint.credential_source([1; 32], 0, || client(&issuer));
        tokio::task::yield_now().await;
        assert!(source().is_none(), "nothing minted yet");

        // The API heals, but the cadence is coarse on purpose: nothing is
        // fetched again before the interval elapses.
        issuer.0.fail_transport.store(false, Ordering::SeqCst);
        tokio::time::advance(Duration::from_secs(599)).await;
        tokio::task::yield_now().await;
        assert_eq!(
            issuer.0.issue_calls.load(Ordering::SeqCst),
            0,
            "the mint must not retry before the interval elapses"
        );

        // The tick at 600 s tops the batch up.
        tokio::time::advance(Duration::from_secs(1)).await;
        let state = issuer.0.clone();
        wait_for(|| state.issue_calls.load(Ordering::SeqCst) >= 1).await;
        assert!(source().is_some(), "the tick must stock the batch");
    }

    #[tokio::test(start_paused = true)]
    async fn a_slot_keeps_one_credential_for_the_whole_epoch_across_reconnects() {
        let issuer = FakeIssuer::new(&[100]);
        let (_t, now) = clock(NOW);
        let mint = EntitlementMint::new(now);
        let source = mint.credential_source([1; 32], 0, || client(&issuer));
        let state = issuer.0.clone();
        wait_for(|| state.issue_calls.load(Ordering::SeqCst) >= 1).await;

        let first = source().expect("a stocked batch vends the slot");
        assert_eq!(
            source().as_deref(),
            Some(first.as_slice()),
            "a renewal must re-present the credential the exit already spent"
        );

        // A reconnect of the same wallet must not build a second manager, and
        // the same slot must keep the same credential.
        let redial = mint.credential_source([1; 32], 0, || unreachable!("manager must be reused"));
        assert_eq!(redial().as_deref(), Some(first.as_slice()));

        // A second slot draws its own credential: two live rules never read as
        // one forwarded port at the exit.
        let other = mint.credential_source([1; 32], 1, || unreachable!("manager must be reused"));
        assert_ne!(other().expect("slot 1 draws its own"), first);
    }

    #[tokio::test]
    async fn the_map_request_carries_the_entitlement_in_its_trailer() {
        // The issuer must publish the epoch the wall clock is in: these two
        // tests run the engine's real refresh loop, so they cannot pause time.
        let issuer = FakeIssuer::new(&[wall_clock_secs() / EPOCH_SECS]);
        let mint = EntitlementMint::new(Arc::new(wall_clock_secs));
        let source = mint.credential_source([1; 32], 0, || client(&issuer));
        let state = issuer.0.clone();
        wait_for(|| state.issue_calls.load(Ordering::SeqCst) >= 1).await;
        let expected = source().expect("a stocked batch vends the slot");

        let datagram = first_map_request(Some(source)).await;
        assert_eq!(
            warrenguard_natpmp_protocol::credential_trailer(&datagram),
            Some(expected.as_slice()),
            "the exit must read this rule's entitlement off the request"
        );
    }

    #[tokio::test]
    async fn an_issuer_with_no_entitlement_leaves_the_request_bare() {
        let issuer = FakeIssuer::new(&[wall_clock_secs() / EPOCH_SECS]);
        issuer.0.refuse_issuance.store(true, Ordering::SeqCst);
        let mint = EntitlementMint::new(Arc::new(wall_clock_secs));
        let source = mint.credential_source([1; 32], 0, || client(&issuer));
        let state = issuer.0.clone();
        wait_for(|| state.issue_calls.load(Ordering::SeqCst) >= 1).await;
        assert!(
            source().is_none(),
            "an issuer that refuses must not fabricate a credential"
        );

        // Degrade, never refuse: the mapping still goes out, and the exit
        // applies its own per-client quota to it.
        let datagram = first_map_request(Some(source)).await;
        assert_eq!(
            datagram.len(),
            warrenguard_natpmp_protocol::MAP_REQUEST_LEN,
            "no entitlement must leave the RFC frame untouched"
        );
        assert_eq!(
            warrenguard_natpmp_protocol::credential_trailer(&datagram),
            None
        );
    }
}
