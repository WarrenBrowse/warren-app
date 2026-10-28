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
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::sync::watch;
use warren_api::{
    BlindingKey, HttpTransport, PersistedTokens, TokenClientError, TokenManager, WarrenApiClient,
};
use warren_app_routes::Credentials;
use warren_app_routes::admission::{RouteKemTrust, remember_route_admission};
use warren_app_routes::tokens::{
    ISSUED_ELSEWHERE_WARNING, IssuedElsewhereNotice, Round, announce_minted, announce_refresh,
    refresh_announcing_mints, refresh_forever,
};
use warren_standing::device::{InstallDevice, settle_round, with_install_device};
use warren_standing::entitlements::RevocationSink;
use warren_standing::token_rotation::with_install_rotation;

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
/// consumed. An empty stack means "no token this epoch": the tokens-only main
/// session does not come up until the refresh brings tokens (matching desktop
/// and iOS).
#[cfg(test)]
pub(crate) type StackSource =
    Arc<dyn Fn() -> Vec<[u8; warrenguard_token::TOKEN_LEN]> + Send + Sync>;

/// Where a refresh failure of a wallet goes, to learn whether it is the
/// issuer's ban refusal (warren-core doc 105 §5.3). Answers whether it was one.
/// Shared with the entitlement mint, whose issuer refuses a banned wallet the
/// same way.
pub(crate) type BanSink = Arc<dyn Fn(&[u8; 32], &TokenClientError) -> bool + Send + Sync>;

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

/// A wallet's public key, and the device its manager asks as.
type WalletKey = ([u8; 32], Option<warren_api::DeviceId>);

/// Process-lived registry of one [`TokenManager`] per wallet and device: a
/// wallet logged in again after its device was revoked asks as a new device,
/// with a new manager.
pub(crate) struct TokenMint<T> {
    now: NowFn,
    managers: parking_lot::Mutex<HashMap<WalletKey, WalletTokens<T>>>,
    persist: Option<Arc<PersistFile>>,
    /// The bundle is restored into the first manager built this process; a
    /// second wallet must not re-load tokens the first already vends.
    restored: AtomicBool,
    ban_sink: Option<BanSink>,
    /// The API server keys trusted to sign the route KEM key of the token
    /// directory (warren-core doc 107 section 6.5), and the file the last
    /// signed route admission block is kept in across a process death.
    route_kem_trust: RouteKemTrust,
    /// Where the install's token rotation is kept
    /// ([`warren_standing::token_rotation::install_rotation`]). Without it each
    /// process draws its own and leads with another serial after a restart.
    rotation_dir: Option<PathBuf>,
    /// The install as a device of its account (warren-core doc 115), which
    /// every manager registers and asks as.
    device: Option<InstallDevice>,
    /// Told once a manager's device was revoked from another device.
    revocation_sink: Option<RevocationSink>,
}

/// One wallet's token manager, and whether it has read the token directory
/// in this process: a restored bundle teaches it the epoch length before
/// any directory read, so the manager alone cannot tell.
pub(crate) struct WalletTokens<T> {
    pub manager: Arc<TokenManager<T>>,
    pub directory_read: Arc<AtomicBool>,
    /// Each finished refresh round, and whether the wallet holds tokens for
    /// the current epoch: a tunnel started before the tokens or the route
    /// admission key were at hand takes them up from here
    /// (`warren_app_routes::main_anchor`).
    pub credentials: watch::Sender<Credentials>,
}

impl<T> Clone for WalletTokens<T> {
    fn clone(&self) -> Self {
        Self {
            manager: Arc::clone(&self.manager),
            directory_read: Arc::clone(&self.directory_read),
            credentials: self.credentials.clone(),
        }
    }
}

impl<T: HttpTransport + 'static> TokenMint<T> {
    pub(crate) fn new(now: NowFn, persist: Option<PersistFile>) -> Self {
        Self {
            now,
            managers: parking_lot::Mutex::new(HashMap::new()),
            persist: persist.map(Arc::new),
            restored: AtomicBool::new(false),
            ban_sink: None,
            route_kem_trust: RouteKemTrust::default(),
            rotation_dir: None,
            device: None,
            revocation_sink: None,
        }
    }

    /// Registers every wallet's manager as `device`: each refresh registers
    /// it, every issue request names it, and the name the registry answers is
    /// kept for the next start.
    pub(crate) fn with_device(mut self, device: InstallDevice) -> Self {
        self.device = Some(device);
        self
    }

    /// Tells `sink` once when the device was revoked; that manager's refresh
    /// then stops for good.
    pub(crate) fn with_revocation_sink(mut self, sink: RevocationSink) -> Self {
        self.revocation_sink = Some(sink);
        self
    }

    /// Keeps the install's token rotation in `dir`, so the next process leads
    /// with the serial this one's sessions were admitted on, and the exit
    /// hands it back its inner address and forwarded ports.
    pub(crate) fn with_rotation_dir(mut self, dir: &Path) -> Self {
        self.rotation_dir = Some(dir.to_path_buf());
        self
    }

    /// Trusts the pins of `trust` to sign the route KEM key, and keeps the
    /// last signed block where it says.
    pub(crate) fn with_route_kem_trust(mut self, trust: RouteKemTrust) -> Self {
        self.route_kem_trust = trust;
        self
    }

    /// Reports every refresh failure to `sink`, which is how an issuer's ban
    /// refusal reaches the standing before any exit is dialed.
    pub(crate) fn with_ban_sink(mut self, sink: BanSink) -> Self {
        self.ban_sink = Some(sink);
        self
    }

    /// The token manager of `wallet_pubkey`. First sight of a wallet builds
    /// it (via `make_client`, which owns the wallet identity, and `key`, the
    /// wallet's session blinding key), restores the persisted bundle into it,
    /// and starts its background refresh; later calls reuse both, so the
    /// factory runs at most once per wallet and process and a later `key` is
    /// dropped unused.
    pub(crate) fn wallet(
        &self,
        wallet_pubkey: [u8; 32],
        key: BlindingKey,
        make_client: impl FnOnce() -> WarrenApiClient<T>,
    ) -> WalletTokens<T> {
        let device_id = self.device.as_ref().map(|d| d.identity().device_id);
        let mut managers = self.managers.lock();
        managers
            .entry((wallet_pubkey, device_id))
            .or_insert_with(|| {
                let manager = Arc::new(
                    with_install_device(
                        with_install_rotation(
                            TokenManager::new(Arc::new(make_client()), key),
                            self.rotation_dir.as_deref(),
                        ),
                        self.device.as_ref(),
                    )
                    .with_mint_horizon(MINT_HORIZON_EPOCHS)
                    .with_server_pubkey_pins(self.route_kem_trust.server_pins.iter().cloned()),
                );
                if let Some(persist) = &self.persist
                    && !self.restored.swap(true, Ordering::SeqCst)
                    && let Some(bundle) = persist.load()
                {
                    let restored = manager.restore_persisted(&bundle);
                    log::info!("restored {restored} persisted v7 tokens");
                }
                let credentials = watch::Sender::new(Credentials::default());
                // Restored tokens of the current epoch are as good as a first
                // round's: the first tunnel of this process does not wait.
                announce_minted(&credentials, &manager, (self.now)());
                let wallet = WalletTokens {
                    manager,
                    directory_read: Arc::new(AtomicBool::new(false)),
                    credentials,
                };
                spawn_refresh(
                    wallet.clone(),
                    self.now.clone(),
                    self.persist.clone(),
                    self.route_kem_trust.remembered_at.clone(),
                    wallet_pubkey,
                    Sinks {
                        ban: self.ban_sink.clone(),
                        revocation: self.revocation_sink.clone(),
                        device: self.device.clone(),
                    },
                );
                wallet
            })
            .clone()
    }

    /// The stack source for `wallet_pubkey`: its manager's current batch,
    /// never consumed, the view the mint tests read the batch through (the
    /// tunnel opens a holding provider per session instead). See
    /// [`Self::wallet`].
    #[cfg(test)]
    pub(crate) fn stack_source(
        &self,
        wallet_pubkey: [u8; 32],
        key: BlindingKey,
        make_client: impl FnOnce() -> WarrenApiClient<T>,
    ) -> StackSource {
        let manager = self.wallet(wallet_pubkey, key, make_client).manager;
        let now = self.now.clone();
        Arc::new(move || {
            let mut stack = manager.session_stack(now());
            stack.truncate(MAX_PRESENTED_TOKENS);
            stack
        })
    }
}

/// Background refresh: the first round runs at once (top up as soon as a
/// wallet is seen), then every 10 minutes, a failed round sooner
/// ([`refresh_forever`]), exactly like the desktop twin. The manager
/// only mints epochs it has not attempted yet, so in steady state a round
/// costs one unsigned directory fetch. Every round is announced on the
/// wallet's credentials once what it read is in force.
/// Where a refresh round's verdicts go besides the store.
struct Sinks {
    ban: Option<BanSink>,
    revocation: Option<RevocationSink>,
    /// Keeps the name the registry answers.
    device: Option<InstallDevice>,
}

fn spawn_refresh<T: HttpTransport + 'static>(
    wallet: WalletTokens<T>,
    now: NowFn,
    persist: Option<Arc<PersistFile>>,
    route_admission_file: Option<PathBuf>,
    wallet_pubkey: [u8; 32],
    sinks: Sinks,
) {
    let sinks = Arc::new(sinks);
    let WalletTokens {
        manager,
        directory_read,
        credentials,
    } = wallet;
    let notice = Arc::new(IssuedElsewhereNotice::default());
    tokio::spawn(refresh_forever(
        move || {
            let manager = Arc::clone(&manager);
            let notice = Arc::clone(&notice);
            let directory_read = Arc::clone(&directory_read);
            let credentials = credentials.clone();
            let now = Arc::clone(&now);
            let persist = persist.clone();
            let route_admission_file = route_admission_file.clone();
            let sinks = Arc::clone(&sinks);
            async move {
                let refreshed = refresh_announcing_mints(&manager, &credentials, &*now).await;
                if settle_round(&manager, sinks.device.as_ref()) {
                    announce_refresh(&credentials, &manager, now());
                    log::info!(
                        "Warren: this device was disconnected from another device of the \
                         account; its token refresh stops"
                    );
                    if let Some(sink) = &sinks.revocation {
                        sink(&wallet_pubkey);
                    }
                    return Round::Over;
                }
                let succeeded = match refreshed {
                    // Log the current-epoch stock so a successful-but-empty
                    // refresh (the issuer refused this account's epoch) is
                    // distinguishable from a mint that actually stocked tokens.
                    // No secret material logged.
                    Ok(()) => {
                        if let Some(persist) = &persist {
                            persist.save(&manager);
                        }
                        // The route admission block this directory carries, or
                        // none: kept for the next process, and in force from now.
                        if let Some(path) = &route_admission_file {
                            remember_route_admission(path, manager.route_admission().as_ref());
                        }
                        directory_read.store(true, Ordering::Release);
                        log::info!(
                            "Warren v7 token refresh ok (current-epoch tokens={})",
                            manager
                                .epoch_at(now())
                                .map_or(0, |epoch| manager.available(epoch))
                        );
                        if notice.due(&manager, now()) {
                            log::warn!("{ISSUED_ELSEWHERE_WARNING}");
                        }
                        true
                    }
                    // A ban refusal is not transient: it goes to the standing,
                    // which blocks the tunnel. Anything else is, and the store
                    // keeps vending what it already holds until the next round
                    // retries. Same message as the desktop twin; the error chain
                    // carries no token or seed material.
                    Err(e) => {
                        if sinks
                            .ban
                            .as_ref()
                            .is_some_and(|sink| sink(&wallet_pubkey, &e))
                        {
                            log::warn!("Warren v7 token refresh refused: the account is banned");
                        } else {
                            log::warn!(
                                "Warren v7 token refresh failed (keeping existing tokens): {e}"
                            );
                        }
                        false
                    }
                };
                announce_refresh(&credentials, &manager, now());
                if succeeded {
                    Round::Refreshed
                } else {
                    Round::Lost
                }
            }
        },
        // No offline-to-online edge reaches this process; the persisted bundle
        // is what spares the first tunnel a wait for its first round.
        None,
    ));
}

#[cfg(all(target_os = "android", feature = "tunnel"))]
pub(crate) use android::{SessionTokens, set_app_files_dir, tokens_for};

#[cfg(all(target_os = "android", feature = "tunnel"))]
mod android {
    use std::sync::{Arc, OnceLock};

    use ed25519_dalek::SigningKey;
    use warren_api::{BlindingKey, WarrenApiClient};
    use warren_app_routes::{
        RouteAdmissionSource, SessionTokenSource,
        admission::{DirectoryRouteAdmission, REMEMBERED_ROUTE_ADMISSION, RouteKemTrust},
        tokens::session_source,
    };
    use warren_identity::WarrenIdentity;

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

    /// The v7 tokens of `signing_key`'s wallet against the product API, for
    /// every session of a tunnel. The minting identity is built from the SAME
    /// Ed25519 key the tunnel handshake signs with
    /// ([`WarrenIdentity::from_signing_key`], no re-derivation), so the
    /// minting wallet is bit-for-bit the subscribed wallet, and `blinding` is
    /// that wallet's session blinding key, so every client of the wallet asks
    /// for the same batch and the issuer serves each of them.
    ///
    /// `source` opens one provider per session (the main one and each route
    /// session), each holding the serial it leads with, so no two sessions of
    /// this device lead with the same serial; a provider hands each dial the
    /// current batch and never mints. `route_admission` is route admission by
    /// anchor as the wallet's token directory announces it (warren-core doc
    /// 107), its key trusted only under a pinned server key's signature, and
    /// the block kept across a process death so a tunnel dialed before the
    /// first directory read still anchors.
    pub(crate) fn tokens_for(signing_key: SigningKey, blinding: BlindingKey) -> SessionTokens {
        let mint = MINT.get_or_init(|| {
            let mint = TokenMint::new(Arc::new(now_unix_secs), persist_file())
                .with_ban_sink(Arc::new(|wallet, error| {
                    crate::standing::store().on_refresh_error(wallet, error, now_unix_secs())
                }))
                .with_route_kem_trust(route_kem_trust());
            let mint = mint.with_revocation_sink(Arc::new(|_| {
                crate::devices::registry().on_revoked();
            }));
            let mint = match crate::devices::registry().device() {
                Some(device) => mint.with_device(device.clone()),
                None => mint,
            };
            match APP_FILES_DIR.get() {
                Some(dir) => mint.with_rotation_dir(dir),
                None => mint,
            }
        });
        let wallet_pubkey = signing_key.verifying_key().to_bytes();
        let wallet = mint.wallet(wallet_pubkey, blinding, move || {
            warren_standing::server_clock::attach(WarrenApiClient::new(
                crate::product::PRODUCT_API_URL.to_owned(),
                WarrenIdentity::from_signing_key(signing_key),
                ProtectedTransport::for_mint(),
            ))
        });
        let route_admission = DirectoryRouteAdmission::new(
            Arc::clone(&wallet.manager),
            route_kem_trust(),
            Arc::new(now_unix_secs),
        )
        .with_directory_read(Arc::clone(&wallet.directory_read));
        SessionTokens {
            source: session_source(wallet.manager, Arc::new(now_unix_secs)),
            route_admission: Arc::new(route_admission),
            credentials: wallet.credentials.subscribe(),
        }
    }

    /// The pins the relay list and the multi-hop directory are verified
    /// against, and the file the last route admission block is kept in.
    fn route_kem_trust() -> RouteKemTrust {
        RouteKemTrust {
            server_pins: crate::product::SERVER_PUBKEY_HEX
                .into_iter()
                .map(str::to_owned)
                .collect(),
            remembered_at: APP_FILES_DIR
                .get()
                .map(|dir| dir.join(REMEMBERED_ROUTE_ADMISSION)),
        }
    }

    /// Every session's tokens, the route admission the directory offers, and
    /// the announcement of each refresh round of the wallet's credentials.
    pub(crate) struct SessionTokens {
        pub source: SessionTokenSource,
        pub route_admission: Arc<dyn RouteAdmissionSource>,
        pub credentials: tokio::sync::watch::Receiver<warren_app_routes::Credentials>,
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

    use warren_app_routes::Credentials;

    use super::{
        MAX_PRESENTED_TOKENS, NowFn, PersistFile, RouteKemTrust, StackSource, TokenMint,
        WalletTokens,
    };

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
        /// Answers every issue request with the issuer's ban refusal
        /// (403 `{"error":"banned"}`, warren-core doc 105 §5.3).
        ban_wallet: AtomicBool,
        fail_transport: AtomicBool,
        issue_calls: AtomicUsize,
        /// The blinded messages of every issue request, in arrival order.
        blinded: parking_lot::Mutex<Vec<Vec<String>>>,
        /// Every request that reached the transport.
        requests: AtomicUsize,
        /// Every device registration, in order.
        registrations: parking_lot::Mutex<Vec<warren_api::DeviceRegistration>>,
        /// The device each issue request named.
        issued_as: parking_lot::Mutex<Vec<Option<warren_api::DeviceId>>>,
        /// Answers registrations and issue requests with the registry's
        /// revocation (403 `{"error":"device_revoked"}`).
        revoke_device: AtomicBool,
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
                ban_wallet: AtomicBool::new(false),
                fail_transport: AtomicBool::new(false),
                issue_calls: AtomicUsize::new(0),
                blinded: parking_lot::Mutex::new(Vec::new()),
                requests: AtomicUsize::new(0),
                registrations: parking_lot::Mutex::new(Vec::new()),
                issued_as: parking_lot::Mutex::new(Vec::new()),
                revoke_device: AtomicBool::new(false),
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
                route_admission: None,
            }
        }
    }

    impl HttpTransport for FakeIssuer {
        async fn execute(&self, request: HttpRequest) -> Result<HttpResponse, TransportError> {
            if self.0.fail_transport.load(Ordering::SeqCst) {
                return Err(TransportError::Connect("fake transport down".to_owned()));
            }
            self.0.requests.fetch_add(1, Ordering::SeqCst);
            let ok = |body: Vec<u8>| Ok(HttpResponse::new(200, body));
            if request.url.ends_with("/v1/tokens/keys") {
                return ok(serde_json::to_vec(&self.directory()).unwrap());
            }
            let revoked = || {
                Ok(HttpResponse::new(
                    403,
                    br#"{"error":"device_revoked"}"#.to_vec(),
                ))
            };
            if request.url.ends_with("/v1/devices") {
                let registration: warren_api::DeviceRegistration =
                    serde_json::from_slice(&request.body).unwrap();
                self.0.registrations.lock().push(registration.clone());
                if self.0.revoke_device.load(Ordering::SeqCst) {
                    return revoked();
                }
                return ok(serde_json::to_vec(&warren_api::DeviceView {
                    device_id: registration.device_id,
                    name: registration.name,
                    platform: registration.platform,
                    app_version: registration.app_version,
                    registered_on: "2026-10-28".to_owned(),
                    last_seen_on: "2026-10-28".to_owned(),
                    this_device: true,
                })
                .unwrap());
            }
            assert!(
                request.url.ends_with("/v1/tokens/issue"),
                "unexpected URL {}",
                request.url
            );
            self.0.issue_calls.fetch_add(1, Ordering::SeqCst);
            if self.0.ban_wallet.load(Ordering::SeqCst) {
                return Ok(HttpResponse::new(
                    403,
                    br#"{"error":"banned","reason_code":"port_forwarding_abuse","lapses_at_unix_secs":2000000000}"#
                        .to_vec(),
                ));
            }
            let req: TokenIssueRequest = serde_json::from_slice(&request.body).unwrap();
            self.0.issued_as.lock().push(req.device_id);
            if self.0.revoke_device.load(Ordering::SeqCst) {
                return revoked();
            }
            self.0
                .blinded
                .lock()
                .extend(req.epochs.iter().map(|e| e.blinded.clone()));
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

    /// Two installs of one wallet ask for the same batch, the one the desktop
    /// app and the extension derive from the wallet seed, so the issuer serves
    /// all of them (warren-core doc 103 section 11). A key blinded from the
    /// tunnel's signing key instead would be a batch no other client sends.
    #[tokio::test(start_paused = true)]
    async fn every_install_of_a_wallet_asks_for_the_batch_its_seed_derives() {
        const MNEMONIC: &str = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let seed = warren_identity::seed_from_mnemonic(MNEMONIC).unwrap();
        let issuer = FakeIssuer::new(&[100]);
        let batch_of = |blinding: BlindingKey| {
            let issuer = issuer.clone();
            async move {
                let mint = TokenMint::new(now_fixed(), None);
                let _source = mint.stack_source([1; 32], blinding, || client(&issuer, WALLET_A));
                let state = issuer.0.clone();
                let before = state.blinded.lock().len();
                wait_for(|| state.blinded.lock().len() > before).await;
                state.blinded.lock().last().cloned().unwrap()
            }
        };

        let first =
            batch_of(crate::wallet::session_blinding_from_mnemonic(MNEMONIC).unwrap()).await;
        let reinstall =
            batch_of(crate::wallet::session_blinding_from_mnemonic(MNEMONIC).unwrap()).await;
        let canonical = batch_of(BlindingKey::session(&seed)).await;
        let from_signing_key = batch_of(BlindingKey::session(
            &warren_identity::derive_node_key(&seed).to_bytes(),
        ))
        .await;

        assert_eq!(first, reinstall);
        assert_eq!(first, canonical, "the batch every other client derives");
        assert_ne!(first, from_signing_key);
    }

    fn now_fixed() -> NowFn {
        Arc::new(|| NOW)
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
        // so the tokens-only main session does not come up.
        assert!(source().is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn the_issuers_ban_refusal_reaches_the_standing_of_the_wallet_it_refused() {
        let issuer = FakeIssuer::new(&[100]);
        issuer.0.ban_wallet.store(true, Ordering::SeqCst);
        let (_t, now) = clock(NOW);
        let standing = Arc::new(warren_standing::StandingStore::new(None));
        let sink = standing.clone();
        let mint = TokenMint::new(now, None).with_ban_sink(Arc::new(move |wallet, error| {
            sink.on_refresh_error(wallet, error, NOW)
        }));
        let _source = source_a(&mint, &issuer);

        wait_for(|| standing.ban_in_force(&[1; 32], NOW).is_some()).await;

        assert_eq!(
            standing
                .ban_in_force(&[1; 32], NOW)
                .and_then(|ban| ban.lapses_at_unix_secs),
            Some(2_000_000_000)
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_refresh_marks_the_directory_read_and_keeps_what_it_says_of_route_admission() {
        let issuer = FakeIssuer::new(&[100]);
        let (_t, now) = clock(NOW);
        let dir = tempfile::tempdir().unwrap();
        let kept = dir.path().join("route-admission.json");
        // A block an earlier process kept, which this directory withdrew.
        std::fs::write(&kept, b"{}").unwrap();
        let mint = TokenMint::new(now, None).with_route_kem_trust(RouteKemTrust {
            server_pins: vec!["00".repeat(32)],
            remembered_at: Some(kept.clone()),
        });
        let wallet = mint.wallet([1; 32], BlindingKey::session(&WALLET_A), || {
            client(&issuer, WALLET_A)
        });
        let before = wallet.directory_read.load(Ordering::Acquire);

        wait_for(|| wallet.directory_read.load(Ordering::Acquire)).await;

        assert!(!before, "a manager that has not read the directory says so");
        assert!(
            !kept.exists(),
            "the next process does not anchor with a withdrawn key"
        );
    }

    fn wallet_a(mint: &TokenMint<FakeIssuer>, issuer: &FakeIssuer) -> WalletTokens<FakeIssuer> {
        mint.wallet([1; 32], BlindingKey::session(&WALLET_A), || {
            client(issuer, WALLET_A)
        })
    }

    #[tokio::test(start_paused = true)]
    async fn each_refresh_round_is_announced_with_the_directory_it_read() {
        let issuer = FakeIssuer::new(&[100]);
        let mint = TokenMint::new(now_fixed(), None);
        let wallet = wallet_a(&mint, &issuer);
        let followed = wallet.credentials.subscribe();

        wait_for(|| followed.borrow().rounds >= 1).await;

        assert_eq!(
            *followed.borrow(),
            Credentials {
                rounds: 1,
                has_tokens: true,
                issued_elsewhere: false,
            }
        );
        assert!(wallet.directory_read.load(Ordering::Acquire));
    }

    /// The first round runs as the first tunnel connects, and a round lost
    /// then left the route sessions without a token for the whole period.
    #[tokio::test(start_paused = true)]
    async fn a_failed_round_is_announced_and_retried_well_before_the_period() {
        let issuer = FakeIssuer::new(&[100]);
        issuer.0.fail_transport.store(true, Ordering::SeqCst);
        let mint = TokenMint::new(now_fixed(), None);
        let wallet = wallet_a(&mint, &issuer);
        let followed = wallet.credentials.subscribe();
        wait_for(|| followed.borrow().rounds >= 1).await;
        assert!(!followed.borrow().has_tokens);

        issuer.0.fail_transport.store(false, Ordering::SeqCst);
        tokio::time::sleep(Duration::from_secs(16)).await;

        assert_eq!(
            *followed.borrow(),
            Credentials {
                rounds: 2,
                has_tokens: true,
                issued_elsewhere: false,
            }
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_restored_bundle_is_announced_before_any_round_ends() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("v7-tokens.json");
        {
            let issuer = FakeIssuer::new(&[100]);
            let mint = TokenMint::new(now_fixed(), Some(PersistFile::new(path.clone())));
            let _source = source_a(&mint, &issuer);
            wait_for(|| path.exists()).await;
        }
        let offline = FakeIssuer::new(&[100]);
        offline.0.fail_transport.store(true, Ordering::SeqCst);
        let mint = TokenMint::new(now_fixed(), Some(PersistFile::new(path)));

        let wallet = wallet_a(&mint, &offline);

        assert_eq!(
            *wallet.credentials.borrow(),
            Credentials {
                rounds: 0,
                has_tokens: true,
                issued_elsewhere: false,
            },
            "a first tunnel of this process has nothing to wait for"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_restarted_process_leads_with_the_token_its_predecessor_led_with() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("v7-tokens.json");
        let batch: Vec<String> = (1..=200u8)
            .map(|fill| {
                let mut token = [fill; warrenguard_token::TOKEN_LEN];
                token[..2].copy_from_slice(&2u16.to_be_bytes());
                BASE64URL_NOPAD.encode(&token)
            })
            .collect();
        let bundle = serde_json::json!({ "epoch_secs": EPOCH_SECS, "epochs": { "100": batch } });
        std::fs::write(&path, bundle.to_string()).unwrap();
        let offline = FakeIssuer::new(&[100]);
        offline.0.fail_transport.store(true, Ordering::SeqCst);
        let lead = || {
            let mint = TokenMint::new(now_fixed(), Some(PersistFile::new(path.clone())))
                .with_rotation_dir(dir.path());
            wallet_a(&mint, &offline).manager.session_stack(NOW)[0]
        };

        assert_eq!(lead(), lead());
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

    /// The install's device kept in `dir`, as Android registers it.
    fn install_device(dir: &std::path::Path) -> warren_standing::device::InstallDevice {
        warren_standing::device::InstallDevice::new(
            dir,
            warren_api::DevicePlatform::Android,
            "1.1.55",
        )
    }

    #[tokio::test(start_paused = true)]
    async fn a_mint_given_the_install_device_registers_it_and_asks_as_it() {
        let dir = tempfile::tempdir().expect("tempdir");
        let kept = install_device(dir.path()).identity();
        let issuer = FakeIssuer::new(&[100]);
        let mint = TokenMint::new(now_fixed(), None).with_device(install_device(dir.path()));

        let source = source_a(&mint, &issuer);
        wait_for(|| !source().is_empty()).await;

        let registered = issuer.0.registrations.lock().clone();
        assert_eq!(
            registered.first().map(|r| (r.device_id, r.platform)),
            Some((kept.device_id, warren_api::DevicePlatform::Android))
        );
        let issued_as = issuer.0.issued_as.lock().clone();
        assert!(!issued_as.is_empty());
        assert!(issued_as.iter().all(|id| *id == Some(kept.device_id)));
    }

    #[tokio::test(start_paused = true)]
    async fn a_revoked_device_is_reported_once_and_its_refresh_stops() {
        let dir = tempfile::tempdir().expect("tempdir");
        let issuer = FakeIssuer::new(&[100]);
        issuer.0.revoke_device.store(true, Ordering::SeqCst);
        let reported = Arc::new(parking_lot::Mutex::new(Vec::new()));
        let sink = Arc::clone(&reported);
        let mint = TokenMint::new(now_fixed(), None)
            .with_device(install_device(dir.path()))
            .with_revocation_sink(Arc::new(move |wallet| sink.lock().push(*wallet)));

        let source = source_a(&mint, &issuer);
        wait_for(|| !reported.lock().is_empty()).await;
        let requests = issuer.0.requests.load(Ordering::SeqCst);
        tokio::time::advance(Duration::from_secs(3 * 600)).await;
        for _ in 0..50 {
            tokio::task::yield_now().await;
        }

        assert_eq!(*reported.lock(), vec![[1; 32]]);
        assert_eq!(
            issuer.0.requests.load(Ordering::SeqCst),
            requests,
            "a revoked device asks nothing more"
        );
        assert!(source().is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn a_device_registered_again_after_its_revocation_gets_tokens_under_its_new_identity() {
        let dir = tempfile::tempdir().expect("tempdir");
        let issuer = FakeIssuer::new(&[100]);
        issuer.0.revoke_device.store(true, Ordering::SeqCst);
        let reported = Arc::new(AtomicUsize::new(0));
        let sink = Arc::clone(&reported);
        let mint = TokenMint::new(now_fixed(), None)
            .with_device(install_device(dir.path()))
            .with_revocation_sink(Arc::new(move |_| {
                sink.fetch_add(1, Ordering::SeqCst);
            }));
        let _revoked = source_a(&mint, &issuer);
        wait_for(|| reported.load(Ordering::SeqCst) == 1).await;

        install_device(dir.path()).forget().expect("forgotten");
        issuer.0.revoke_device.store(false, Ordering::SeqCst);
        let again = source_a(&mint, &issuer);
        wait_for(|| !again().is_empty()).await;

        let registered = issuer.0.registrations.lock().clone();
        assert_ne!(
            registered.first().map(|r| r.device_id),
            registered.last().map(|r| r.device_id)
        );
    }
}
