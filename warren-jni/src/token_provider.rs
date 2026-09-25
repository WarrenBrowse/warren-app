//! v7 anonymous session credentials (Privacy Pass, warren-core doc 64) for the Android
//! tunnel: the warren-jni twin of the desktop daemon's
//! `mullvad-daemon::warren_token_provider` and of iOS's
//! `warren-ios::warren_token_provider`.
//!
//! One [`warren_api::TokenManager`] per wallet, process-lived: built the first
//! time a wallet connects and reused across reconnects, so its RAM token store
//! and its once-per-epoch issuance bookkeeping survive between sessions. A
//! background task tops it up (current epoch plus [`MINT_HORIZON_EPOCHS`]) on
//! a coarse timer. A dial never mints: minting at connect time would let the
//! issuer correlate the wallet's mint request with the exit's spend and
//! re-link what the blind signature unlinks.
//!
//! The batch is blinded from the wallet seed ([`BlindingKey`]), so every
//! client of the wallet holds the same tokens and the issuer serves the batch
//! again to whoever asks for it with the same blinding. An exit leases each
//! serial to one live session in the whole fleet, so a dial is handed the
//! whole current-epoch batch and the engine walks it, redialling with the next
//! token when the exit refuses one. Nothing is consumed: a redial costs no
//! token.
//!
//! Android lifecycle: the VpnService process is killed and restarted by the
//! system routinely. The store is persisted as the SDK's seed-free
//! [`warren_api::PersistedTokens`] bundle in app-private storage
//! (`allowBackup=false`, so it never reaches a cloud backup) after each
//! refresh and restored at process start, so the first dial after a process
//! death is v7 before the issuer has answered the replacement process; the
//! bundle carries only anonymous bearer tokens, never the seed, the wallet or
//! anything the token structure does not already carry. Minting is capped at
//! [`MINT_HORIZON_EPOCHS`] ahead instead of the whole published window, which
//! bounds the bearer value at rest and the signed requests a launch costs.
//!
//! Tokens still never cross the JNI boundary into Kotlin.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use warren_api::{BlindingKey, HttpTransport, PersistedTokens, TokenManager, WarrenApiClient};
use warrenguard_token::TOKEN_LEN;

/// The most tokens one setup request may carry: the default admission sends
/// the whole stack at once, and the wire refuses to decode a request with
/// more.
const MAX_PRESENTED_TOKENS: usize = 8;

#[cfg(all(target_os = "android", feature = "tunnel"))]
const _: () = assert!(MAX_PRESENTED_TOKENS == warrenguard_wire::MAX_SESSION_TOKENS);

/// Epochs minted ahead of the current one (current + horizon per refresh
/// tick). With hourly issuer epochs and the 10 minute refresh cadence this
/// keeps ~3 h of runway through an API outage while bounding what a lost or
/// stolen bundle is worth. The issuer evaluates each requested epoch
/// independently, so the narrowed request needs no server-side change, and it
/// serves a derived batch again, so a narrow window loses nothing across a
/// process death.
const MINT_HORIZON_EPOCHS: u64 = 2;

/// Unix-seconds clock seam. The clock is a system boundary (shared TDD rule):
/// tests drive epochs deterministically, production wires the system clock.
type NowFn = Arc<dyn Fn() -> u64 + Send + Sync>;

/// Source of the per-session token stack, in the serialized form the tunnel
/// layer wraps into wire `SessionToken`s: the current epoch's batch, never
/// consumed. An empty stack means "no token this epoch" and keeps the v6
/// wallet-signed path (availability over a temporary anonymity downgrade,
/// matching desktop and iOS).
pub(crate) type StackSource = Arc<dyn Fn() -> Vec<[u8; TOKEN_LEN]> + Send + Sync>;

/// The on-disk home of the seed-free token bundle, written atomically
/// (temp + rename, the same pattern as the iOS TOFU pin store) so a process
/// death mid-write can never corrupt the restorable state.
pub(crate) struct PersistFile {
    path: PathBuf,
}

impl PersistFile {
    pub(crate) fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// Best-effort load: a missing, unreadable or malformed file simply means
    /// nothing to restore (first run, or a cleared app storage).
    fn load(&self) -> Option<PersistedTokens> {
        let body = std::fs::read_to_string(&self.path).ok()?;
        PersistedTokens::from_json(&body).ok()
    }

    /// Snapshots the manager's store to disk. Failures are logged and
    /// swallowed: persistence is an availability optimisation, never worth
    /// failing a session over. No token material reaches the log.
    fn save<T: HttpTransport>(&self, manager: &TokenManager<T>) {
        let Some(bundle) = manager.export_persistable() else {
            return;
        };
        let Ok(json) = bundle.to_json() else {
            return;
        };
        let tmp = self
            .path
            .with_extension(format!("tmp.{}", std::process::id()));
        let written = std::fs::write(&tmp, json).and_then(|()| {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
            }
            std::fs::rename(&tmp, &self.path)
        });
        if let Err(e) = written {
            let _ = std::fs::remove_file(&tmp);
            log::warn!("v7 token bundle persist failed: {}", e.kind());
        }
    }
}

/// Process-lived registry of one [`TokenManager`] per wallet.
pub(crate) struct TokenMint<T> {
    now: NowFn,
    managers: parking_lot::Mutex<HashMap<[u8; 32], Arc<TokenManager<T>>>>,
    persist: Option<Arc<PersistFile>>,
    /// The bundle is restored into the first manager built this process; a
    /// second wallet must not re-load tokens the first already vends.
    restored: AtomicBool,
}

impl<T: HttpTransport + 'static> TokenMint<T> {
    pub(crate) fn new(now: NowFn, persist: Option<PersistFile>) -> Self {
        Self {
            now,
            managers: parking_lot::Mutex::new(HashMap::new()),
            persist: persist.map(Arc::new),
            restored: AtomicBool::new(false),
        }
    }

    /// The stack source for `wallet_pubkey`. First sight of a wallet builds
    /// its manager (via `make_client`, which owns the wallet identity, and
    /// `key`, the wallet's session blinding key), restores the persisted
    /// bundle into it, and starts its background refresh; later calls reuse
    /// both, so the factory runs at most once per wallet and process and a
    /// later `key` is dropped unused.
    pub(crate) fn stack_source(
        &self,
        wallet_pubkey: [u8; 32],
        key: BlindingKey,
        make_client: impl FnOnce() -> WarrenApiClient<T>,
    ) -> StackSource {
        let manager = {
            let mut managers = self.managers.lock();
            managers
                .entry(wallet_pubkey)
                .or_insert_with(|| {
                    let manager = Arc::new(
                        TokenManager::new(Arc::new(make_client()), key)
                            .with_mint_horizon(MINT_HORIZON_EPOCHS),
                    );
                    if let Some(persist) = &self.persist
                        && !self.restored.swap(true, Ordering::SeqCst)
                        && let Some(bundle) = persist.load()
                    {
                        let restored = manager.restore_persisted(&bundle);
                        log::info!("restored {restored} persisted v7 tokens");
                    }
                    spawn_refresh(manager.clone(), self.now.clone(), self.persist.clone());
                    manager
                })
                .clone()
        };
        let now = self.now.clone();
        Arc::new(move || {
            let mut stack = manager.session_stack(now());
            stack.truncate(MAX_PRESENTED_TOKENS);
            stack
        })
    }
}

/// Background refresh: the first tick fires immediately (top up as soon as a
/// wallet is seen), then every 10 minutes, exactly like the desktop and iOS
/// twins. The manager only mints epochs it has not attempted yet, so in steady
/// state a tick costs one unsigned directory fetch.
fn spawn_refresh<T: HttpTransport + 'static>(
    manager: Arc<TokenManager<T>>,
    now: NowFn,
    persist: Option<Arc<PersistFile>>,
) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(600));
        loop {
            tick.tick().await;
            let n = now();
            match manager.refresh(n).await {
                // Log the current-epoch stock so a successful-but-empty refresh
                // (the issuer refused this account's epoch) is distinguishable
                // from a mint that actually stocked tokens. No secret material
                // logged.
                Ok(()) => {
                    if let Some(persist) = &persist {
                        persist.save(&manager);
                    }
                    log::info!(
                        "Warren v7 token refresh ok (current-epoch tokens={})",
                        manager
                            .epoch_at(n)
                            .map_or(0, |epoch| manager.available(epoch))
                    );
                }
                // Transient: the store keeps vending what it already holds and
                // the next tick retries. Same message as the desktop twin; the
                // error chain carries no token or seed material.
                Err(e) => {
                    log::warn!("Warren v7 token refresh failed (keeping existing tokens): {e}")
                }
            }
        }
    });
}

#[cfg(all(target_os = "android", feature = "tunnel"))]
pub(crate) use android::{provider_for, set_app_files_dir};

#[cfg(all(target_os = "android", feature = "tunnel"))]
mod android {
    use std::sync::{Arc, OnceLock};

    use ed25519_dalek::SigningKey;
    use warren_api::{BlindingKey, WarrenApiClient};
    use warren_identity::WarrenIdentity;
    use warrenguard_transport::supervisor::SessionTokenProvider;
    use warrenguard_wire::SessionToken;

    use super::TokenMint;
    use crate::protected_transport::ProtectedTransport;

    /// Process-lived mint registry: survives connect/disconnect cycles so the
    /// refresh cadence stays decoupled from session timing (see module docs
    /// for the process-death story). The transport is the VpnService-protected
    /// one: an unprotected mint socket loses the tunnel bring-up race (routed
    /// into the not-yet-passing TUN), which made the first session per process
    /// v6.
    static MINT: OnceLock<TokenMint<ProtectedTransport>> = OnceLock::new();

    /// The app-private files directory, captured at `initLogger` time (the
    /// only JNI call that carries it), so the token bundle has a home before
    /// the first connect.
    static APP_FILES_DIR: OnceLock<std::path::PathBuf> = OnceLock::new();

    /// Records the app-private files directory the persisted token bundle
    /// lives in. Idempotent; first caller wins.
    pub(crate) fn set_app_files_dir(dir: &std::path::Path) {
        let _ = APP_FILES_DIR.set(dir.to_path_buf());
    }

    fn persist_file() -> Option<super::PersistFile> {
        // No captured dir (initLogger never ran) degrades to the RAM-only
        // behavior rather than failing the mint entirely.
        let dir = APP_FILES_DIR.get()?;
        Some(super::PersistFile::new(dir.join("v7-tokens.json")))
    }

    fn now_unix_secs() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }

    /// The v7 token provider for `signing_key`'s wallet against the production
    /// API. The minting identity is built from the SAME Ed25519 key the tunnel
    /// handshake signs with ([`WarrenIdentity::from_signing_key`], no
    /// re-derivation), so the minting wallet is bit-for-bit the subscribed
    /// wallet, and `blinding` is that wallet's session blinding key. The
    /// returned closure hands each dial the current batch and never mints.
    pub(crate) fn provider_for(
        signing_key: SigningKey,
        blinding: BlindingKey,
    ) -> SessionTokenProvider {
        let mint = MINT.get_or_init(|| TokenMint::new(Arc::new(now_unix_secs), persist_file()));
        let wallet_pubkey = signing_key.verifying_key().to_bytes();
        let source = mint.stack_source(wallet_pubkey, blinding, move || {
            WarrenApiClient::new(
                crate::product::PRODUCT_API_URL.to_owned(),
                WarrenIdentity::from_signing_key(signing_key),
                ProtectedTransport::new(),
            )
        });
        Arc::new(move || source().into_iter().map(SessionToken).collect())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
    use std::time::Duration;

    use data_encoding::BASE64URL_NOPAD;
    use rand010::SeedableRng;
    use rand010::rngs::StdRng;
    use warren_api::transport::{HttpRequest, HttpResponse, HttpTransport, TransportError};
    use warren_api::{
        BlindingKey, TokenEpochResponse, TokenIssueRequest, TokenIssueResponse,
        TokenIssuerDirectory, TokenIssuerKey, WarrenApiClient,
    };
    use warren_identity::WarrenIdentity;
    use warrenguard_token::IssuerSecretKey;

    use super::{MAX_PRESENTED_TOKENS, NowFn, PersistFile, StackSource, TokenMint};

    const EPOCH_SECS: u64 = 3600;
    const QUOTA: u32 = 3;
    const NOW: u64 = 100 * EPOCH_SECS + 5;
    const WALLET_A: [u8; 32] = [0x33; 32];
    const WALLET_B: [u8; 32] = [0x44; 32];

    /// Observable state of the fake issuer, shared with the test body. The
    /// HTTP transport is the mocked system boundary (as in warren-api's own
    /// `tokens_mint` suite); the blind-RSA crypto is the real engine code, so
    /// vended tokens are real tokens.
    struct IssuerState {
        keys: HashMap<u64, IssuerSecretKey>,
        quota: u32,
        refuse_issuance: AtomicBool,
        fail_transport: AtomicBool,
        issue_calls: AtomicUsize,
    }

    #[derive(Clone)]
    struct FakeIssuer(Arc<IssuerState>);

    impl FakeIssuer {
        fn new(epochs: &[u64]) -> Self {
            Self::with_quota(epochs, QUOTA)
        }

        fn with_quota(epochs: &[u64], quota: u32) -> Self {
            let mut rng = StdRng::seed_from_u64(4242);
            let keys = epochs
                .iter()
                .map(|&e| (e, IssuerSecretKey::generate(&mut rng).unwrap()))
                .collect();
            Self(Arc::new(IssuerState {
                keys,
                quota,
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
                quota_per_epoch: self.0.quota,
                prefetch_epochs: 48,
                keys,
                attribution_verifying_key_hex: None,
            }
        }
    }

    impl HttpTransport for FakeIssuer {
        async fn execute(&self, request: HttpRequest) -> Result<HttpResponse, TransportError> {
            if self.0.fail_transport.load(Ordering::SeqCst) {
                return Err(TransportError::Connect("fake transport down".to_owned()));
            }
            let ok = |body: Vec<u8>| Ok(HttpResponse { status: 200, body });
            if request.url.ends_with("/v1/tokens/keys") {
                return ok(serde_json::to_vec(&self.directory()).unwrap());
            }
            assert!(
                request.url.ends_with("/v1/tokens/issue"),
                "unexpected URL {}",
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
                        attribution_tags: Vec::new(),
                    });
                    continue;
                }
                let sk = self.0.keys.get(&e.epoch).expect("key for requested epoch");
                let sigs = e
                    .blinded
                    .iter()
                    .map(|b| {
                        let bytes = BASE64URL_NOPAD.decode(b.as_bytes()).unwrap();
                        BASE64URL_NOPAD.encode(&sk.blind_sign(&bytes).unwrap())
                    })
                    .collect();
                epochs.push(TokenEpochResponse {
                    epoch: e.epoch,
                    issued: true,
                    blind_signatures: sigs,
                    token_key_id: Some(sk.public_key().key_id().to_hex()),
                    reject_reason: None,
                    attribution_tags: Vec::new(),
                });
            }
            ok(serde_json::to_vec(&TokenIssueResponse { epochs }).unwrap())
        }
    }

    fn client(issuer: &FakeIssuer, wallet: [u8; 32]) -> WarrenApiClient<FakeIssuer> {
        WarrenApiClient::new(
            "https://api.example.test",
            WarrenIdentity::from_seed(&wallet),
            issuer.clone(),
        )
    }

    /// Wallet A's source: its identity and its session blinding key.
    fn source_a(mint: &TokenMint<FakeIssuer>, issuer: &FakeIssuer) -> StackSource {
        mint.stack_source([1; 32], BlindingKey::session(&WALLET_A), || {
            client(issuer, WALLET_A)
        })
    }

    /// A movable epoch clock: the handle steps time, the `NowFn` reads it.
    fn clock(start: u64) -> (Arc<AtomicU64>, NowFn) {
        let t = Arc::new(AtomicU64::new(start));
        let read = t.clone();
        (t, Arc::new(move || read.load(Ordering::SeqCst)))
    }

    /// Bounded wait for the background refresh task to reach `cond`.
    async fn wait_for(mut cond: impl FnMut() -> bool) {
        for _ in 0..1000 {
            if cond() {
                return;
            }
            tokio::task::yield_now().await;
        }
        panic!("background refresh never reached the expected state");
    }

    #[tokio::test(start_paused = true)]
    async fn first_wallet_sight_mints_in_the_background_and_every_dial_draws_the_batch() {
        let issuer = FakeIssuer::new(&[100, 101]);
        let (_t, now) = clock(NOW);
        let mint = TokenMint::new(now, None);
        let source = source_a(&mint, &issuer);

        // The background refresh (not any dial) fills the store: one issue
        // request per published epoch.
        let state = issuer.0.clone();
        wait_for(|| state.issue_calls.load(Ordering::SeqCst) >= 2).await;

        // Every dial is handed the whole batch: a dial spends nothing.
        let first = source();
        assert_eq!(first.len(), QUOTA as usize);
        assert_eq!(source(), first, "a redial is handed the same batch");

        // A dial must never trigger a mint (issuance timing would mirror
        // session timing and re-link wallet and exit).
        let calls = state.issue_calls.load(Ordering::SeqCst);
        for _ in 0..3 {
            let _ = source();
        }
        assert_eq!(state.issue_calls.load(Ordering::SeqCst), calls);
    }

    #[tokio::test(start_paused = true)]
    async fn a_dial_presents_no_more_tokens_than_one_setup_may_carry() {
        let issuer = FakeIssuer::with_quota(&[100], 12);
        let (_t, now) = clock(NOW);
        let mint = TokenMint::new(now, None);
        let source = source_a(&mint, &issuer);
        let state = issuer.0.clone();
        wait_for(|| state.issue_calls.load(Ordering::SeqCst) >= 1).await;

        assert_eq!(source().len(), MAX_PRESENTED_TOKENS);
    }

    #[tokio::test(start_paused = true)]
    async fn reconnects_reuse_the_wallet_manager_and_its_stock() {
        let issuer = FakeIssuer::new(&[100]);
        let (_t, now) = clock(NOW);
        let mint = TokenMint::new(now, None);
        let first = source_a(&mint, &issuer);
        let state = issuer.0.clone();
        wait_for(|| state.issue_calls.load(Ordering::SeqCst) >= 1).await;

        // A reconnect of the same wallet must not build a second manager (the
        // factory does not run) and draws the same stock.
        let second = mint.stack_source([1; 32], BlindingKey::session(&WALLET_A), || {
            unreachable!("manager must be reused")
        });
        assert_eq!(second(), first());
    }

    #[tokio::test(start_paused = true)]
    async fn distinct_wallets_hold_batches_of_their_own() {
        let a = FakeIssuer::new(&[100]);
        let b = FakeIssuer::new(&[100]);
        let (_t, now) = clock(NOW);
        let mint = TokenMint::new(now, None);
        let source_a = source_a(&mint, &a);
        let source_b = mint.stack_source([2; 32], BlindingKey::session(&WALLET_B), || {
            client(&b, WALLET_B)
        });
        let (sa, sb) = (a.0.clone(), b.0.clone());
        wait_for(move || {
            sa.issue_calls.load(Ordering::SeqCst) >= 1 && sb.issue_calls.load(Ordering::SeqCst) >= 1
        })
        .await;

        let (stack_a, stack_b) = (source_a(), source_b());
        assert_eq!(
            stack_b.len(),
            QUOTA as usize,
            "wallet B keeps its own stock"
        );
        assert!(stack_a.iter().all(|token| !stack_b.contains(token)));
    }

    #[tokio::test(start_paused = true)]
    async fn refresh_failure_keeps_already_minted_tokens() {
        let issuer = FakeIssuer::new(&[100]);
        let (_t, now) = clock(NOW);
        let mint = TokenMint::new(now, None);
        let source = source_a(&mint, &issuer);
        let state = issuer.0.clone();
        wait_for(|| state.issue_calls.load(Ordering::SeqCst) >= 1).await;

        // Later refresh ticks fail at the transport: the store must keep
        // vending what it already minted, not clear.
        state.fail_transport.store(true, Ordering::SeqCst);
        tokio::time::advance(Duration::from_secs(600)).await;
        tokio::task::yield_now().await;
        assert_eq!(source().len(), QUOTA as usize);
    }

    #[tokio::test(start_paused = true)]
    async fn refused_issuance_yields_the_empty_stack_v6_fallback() {
        let issuer = FakeIssuer::new(&[100]);
        issuer.0.refuse_issuance.store(true, Ordering::SeqCst);
        let (_t, now) = clock(NOW);
        let mint = TokenMint::new(now, None);
        let source = source_a(&mint, &issuer);
        let state = issuer.0.clone();
        wait_for(|| state.issue_calls.load(Ordering::SeqCst) >= 1).await;

        // The issuer answered but refused (wallet not subscribed): no token,
        // the dial rides the v6 wallet-signed path.
        assert!(source().is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn before_the_first_refresh_lands_the_stack_is_empty_v6() {
        let issuer = FakeIssuer::new(&[100]);
        issuer.0.fail_transport.store(true, Ordering::SeqCst);
        let (_t, now) = clock(NOW);
        let mint = TokenMint::new(now, None);
        let source = source_a(&mint, &issuer);

        // The API is unreachable: every dial stays on the v6 path, no panic.
        assert!(source().is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn epoch_rollover_vends_prefetched_tokens_then_runs_dry() {
        let issuer = FakeIssuer::new(&[100, 101]);
        let (t, now) = clock(NOW);
        let mint = TokenMint::new(now, None);
        let source = source_a(&mint, &issuer);
        let state = issuer.0.clone();
        wait_for(|| state.issue_calls.load(Ordering::SeqCst) >= 2).await;
        let current = source();

        // Epoch rollover: the prefetched next-epoch batch takes over without
        // any new mint (the background refresh already stocked it).
        t.store(101 * EPOCH_SECS + 1, Ordering::SeqCst);
        let next = source();
        assert_eq!(next.len(), QUOTA as usize);
        assert!(next.iter().all(|token| !current.contains(token)));

        // Beyond the published window nothing is spendable: empty stack (v6).
        t.store(103 * EPOCH_SECS + 1, Ordering::SeqCst);
        assert!(source().is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn the_mint_horizon_caps_the_background_prefetch() {
        // The issuer publishes a 50-epoch window; the Android policy must ask
        // only for current..=current+MINT_HORIZON_EPOCHS.
        let published: Vec<u64> = (100..150).collect();
        let issuer = FakeIssuer::new(&published);
        let (_t, now) = clock(NOW);
        let mint = TokenMint::new(now, None);
        let _source = source_a(&mint, &issuer);

        let state = issuer.0.clone();
        wait_for(|| state.issue_calls.load(Ordering::SeqCst) >= 3).await;
        for _ in 0..200 {
            tokio::task::yield_now().await;
        }
        assert_eq!(
            state.issue_calls.load(Ordering::SeqCst),
            3,
            "only the current epoch plus the horizon may be minted"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_replacement_process_restores_the_bundle_and_dials_v7_offline() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("v7-tokens.json");

        // First process: mints and persists after its refresh.
        let issuer = FakeIssuer::new(&[100]);
        let first_batch = {
            let (_t, now) = clock(NOW);
            let mint = TokenMint::new(now, Some(PersistFile::new(path.clone())));
            let source = source_a(&mint, &issuer);
            let state = issuer.0.clone();
            wait_for(|| state.issue_calls.load(Ordering::SeqCst) >= 1).await;
            wait_for(|| path.exists()).await;
            source()
        };

        // Replacement process with the transport down: the restored bundle
        // alone must make the first dial v7.
        let offline = FakeIssuer::new(&[100]);
        offline.0.fail_transport.store(true, Ordering::SeqCst);
        let (_t, now) = clock(NOW);
        let mint = TokenMint::new(now, Some(PersistFile::new(path)));
        let source = source_a(&mint, &offline);
        let mut restored = source();
        let mut first_batch = first_batch;
        restored.sort_unstable();
        first_batch.sort_unstable();
        assert_eq!(
            restored, first_batch,
            "the first dial after process death draws the restored batch"
        );
    }
}
