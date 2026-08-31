//! Periodic + on-startup updater for the signed Warren exit list.
//!
//! Mirrors the upstream Mullvad [`crate::relay_list::RelayListUpdater`]
//! pattern (cheap periodic check, staleness gate, ETag conditional GET,
//! exponential backoff, atomic cache write, push to the live selector),
//! but targets warren-api's public signed endpoint
//! `GET {api_url}/v1/exits` (format v3 `SignedRelayList`, Ed25519).
//!
//! Security invariants:
//! - The fetched body is **signature-verified against the pinned server
//!   pubkey BEFORE** it is ever written to the on-disk cache or pushed to
//!   the selector. A forged/unsigned/tampered list never touches state.
//! - A failed fetch (network, TLS, 5xx, bad signature) **never clobbers**
//!   the last-good cache: we keep serving what we already had.
//! - The pinned pubkey comes from the build-time-baked bootstrap (see
//!   [`load_bootstrap`]); a future list signed by a different key is
//!   rejected (anti-MITM / anti-key-swap).

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures::channel::mpsc;
use futures::future::{Fuse, FusedFuture};
use futures::{FutureExt, SinkExt, StreamExt};

use talpid_future::retry::{ExponentialBackoff, Jittered, retry_future};
use warren_discovery_core::{
    RosterError, SignedError, VerifiedRelayList, VerifiedRoster, WarrenRelayList,
    verify_roster_any, verify_signed_relay_list_any,
};

use crate::warren_artifact_refresh::{
    FETCH_TIMEOUT, FetchResponse, conditional_get, now_unix, split_pins,
};
use crate::warren_relay_selector::WARREN_RELAYS_FILENAME;

/// How often the updater wakes up to consider refreshing. Cheap: the
/// staleness gate ([`UPDATE_INTERVAL`]) decides whether a fetch fires.
const UPDATE_CHECK_INTERVAL: Duration = Duration::from_secs(15 * 60);
/// How old the list must be before a periodic refresh is triggered.
const UPDATE_INTERVAL: Duration = Duration::from_secs(60 * 60);

/// Build the reqwest client used for roster fetches.
///
/// The API host is resolved from the daemon's address cache
/// ([`crate::warren_api_dns`]), so a roster refresh still reaches the API
/// while the blocking state drops system DNS.
///
/// When an API IP is pinned ([`mullvad_api_constants::API_PINNED_IP`]) *and*
/// `api_url` targets the default API host, resolve that host to the pinned IP
/// (no DNS query) and omit TLS SNI - bootstrap-privacy parity with the main
/// API client (`mullvad-api`). The host guard avoids mis-pinning when the
/// operator overrides the API URL to a different host. `API_PINNED_IP` is
/// `None` by default, so the pin arm is dormant until the server-side
/// dedicated-IP prerequisite is live.
fn build_http_client(api_url: &str) -> reqwest::Client {
    use mullvad_api_constants::{API_HOST_DEFAULT, API_PINNED_IP};

    let mut builder =
        crate::warren_api_dns::with_api_resolver(reqwest::Client::builder().timeout(FETCH_TIMEOUT));
    if let Some(addr) = pin_target(api_url, API_PINNED_IP) {
        // No DNS query (resolve the host to the pinned IP) and no SNI on the
        // wire - the dedicated endpoint presents the cert without SNI.
        builder = builder.resolve(API_HOST_DEFAULT, addr).tls_sni(false);
    }
    // expect, not a fallback: reqwest::Client::new() would silently drop the
    // pinned `.resolve()` + `.tls_sni(false)` above and leak a DNS query + SNI.
    builder
        .build()
        .expect("reqwest client build failed: invalid TLS backend configuration")
}

/// Pure helper (testable): the pinned `SocketAddr` to dial when `api_url`
/// targets the default API host and an IP is pinned, else `None`. The
/// host guard prevents mis-pinning when the API URL is overridden.
fn pin_target(api_url: &str, pinned: Option<std::net::IpAddr>) -> Option<std::net::SocketAddr> {
    use mullvad_api_constants::{API_HOST_DEFAULT, API_PORT_DEFAULT};
    let ip = pinned?;
    let url = reqwest::Url::parse(api_url).ok()?;
    if url.host_str()?.eq_ignore_ascii_case(API_HOST_DEFAULT) {
        Some(std::net::SocketAddr::new(ip, API_PORT_DEFAULT))
    } else {
        None
    }
}

/// Exponential backoff on repeated download failures so a flapping API or
/// offline client does not hammer the network or flood the logs.
const DOWNLOAD_RETRY_STRATEGY: Jittered<ExponentialBackoff> = Jittered::jitter(
    ExponentialBackoff::new(Duration::from_secs(16), 8).max_delay(Some(Duration::from_secs(7200))),
);

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("HTTP request failed")]
    Http(#[from] reqwest::Error),

    #[error("server returned non-success status {0}")]
    Status(u16),

    #[error("signature/format verification failed")]
    Verify(#[from] SignedError),

    #[error("roster verification failed")]
    RosterVerify(#[from] RosterError),

    #[error("failed to write cache at {0}")]
    Io(String, #[source] std::io::Error),
}

/// Outcome of a single conditional fetch.
#[derive(Debug)]
pub enum FetchOutcome {
    /// Server replied `304 Not Modified`: the cached list is current.
    NotModified,
    /// A new, signature-verified list was fetched. Signature is checked
    /// here; the freshness gate (monotonic generation + expiry) is applied
    /// later by [`freshness_check`] in `consume` (it needs `now` + the
    /// updater's high-water mark).
    Updated {
        /// Raw signed JSON body (to persist verbatim in the cache).
        raw: String,
        /// `ETag` header to send as `If-None-Match` on the next request.
        etag: Option<String>,
        /// Parsed + verified list plus its freshness metadata
        /// (`generation`, `expires_at`).
        verified: VerifiedRelayList,
    },
}

/// Why a signature-valid fetched list was nonetheless rejected by the
/// freshness gate (TUF rollback/freeze defense).
#[derive(Debug, PartialEq, Eq)]
pub enum FreshnessReject {
    /// The signed `expires_at` is at or before `now`: a replayed/stale
    /// list (freeze attack), or a clock far ahead. Reject on the live
    /// path; keep the last-good list.
    Expired { expires_at: u64, now: u64 },
    /// The list's `generation` is lower than the highest already trusted:
    /// a rollback attempt (or a CDN serving an out-of-date copy).
    Rollback { got: u64, have: u64 },
}

/// Pure freshness gate (no clock, no I/O): rejects an expired list
/// (freeze defense) or one whose `generation` is below the high-water
/// mark (rollback defense). Per TUF, BOTH checks are required - a signed
/// creation timestamp alone defeats neither attack.
///
/// `generation == highest_generation` is **accepted** (idempotent
/// re-fetch of the current list); only a strictly lower generation is a
/// rollback.
///
/// # Errors
/// [`FreshnessReject`] describing which guard tripped.
pub fn freshness_check(
    verified: &VerifiedRelayList,
    highest_generation: u64,
    now: u64,
) -> Result<(), FreshnessReject> {
    if verified.is_expired(now) {
        return Err(FreshnessReject::Expired {
            expires_at: verified.expires_at,
            now,
        });
    }
    if verified.generation < highest_generation {
        return Err(FreshnessReject::Rollback {
            got: verified.generation,
            have: highest_generation,
        });
    }
    Ok(())
}

/// Outcome of applying the offline-admin roster to a signature-verified
/// live list.
#[derive(Debug)]
pub struct RosterEnforcement {
    /// The list actually published to the selector.
    pub list: WarrenRelayList,
    /// How many live-list exits were dropped as un-authorized.
    pub dropped: usize,
    /// `true` if a roster was present and enforced; `false` if no roster
    /// was available and the list passed through (rollout transition).
    pub enforced: bool,
}

/// Applies the offline-admin roster to the (online-signed) live list.
///
/// With a verified roster present, only roster-authorized exits survive:
/// a compromised online backend cannot inject a new exit, relocate one to
/// another country, or swap its pubkey - all such relays are dropped
/// (the core anti-backend-compromise property).
///
/// When **no** roster is available (never fetched and none baked - i.e.
/// during the rollout before rosters are deployed), the list passes
/// through unfiltered with `enforced = false`. This fail-**open** is a
/// deliberate transitional choice so a client predating roster deployment
/// is not bricked; the caller logs a loud warning. Once any verified
/// roster exists, enforcement is mandatory.
#[must_use]
pub fn enforce_roster(list: WarrenRelayList, roster: Option<&VerifiedRoster>) -> RosterEnforcement {
    match roster {
        Some(r) => {
            let res = r.authorize(&list);
            RosterEnforcement {
                list: res.authorized,
                dropped: res.dropped,
                enforced: true,
            }
        }
        None => RosterEnforcement {
            list,
            dropped: 0,
            enforced: false,
        },
    }
}

/// Availability guard: whether to publish a freshly resolved list.
/// Refuses to replace a known-good non-empty served list with an empty
/// one (transient drain / roster all-dropped). The *first* empty (before
/// any non-empty was ever served) is allowed so the UI can show "no exits
/// yet" rather than stale data.
#[must_use]
pub fn should_publish(new_is_empty: bool, published_nonempty_before: bool) -> bool {
    !(new_is_empty && published_nonempty_before)
}

/// Outcome of a single conditional roster fetch.
#[derive(Debug)]
enum RosterFetch {
    /// `304` (unchanged) or `404` (none published yet): keep last-good.
    NotModifiedOrAbsent,
    /// A new, signature-verified roster.
    Updated {
        etag: Option<String>,
        verified: VerifiedRoster,
    },
}

/// Pure decision step: given an HTTP status, body, ETag and the pinned
/// server pubkey, decide the [`FetchOutcome`]. Factored out of the
/// network call so it is unit-testable without a server.
///
/// `200` bodies are **signature-verified** (signature + pinned pubkey +
/// v4 format) here; verification failure is surfaced as [`Error::Verify`]
/// and the caller keeps the previous cache. The freshness gate (monotonic
/// `generation` + `expires_at`) is applied separately by
/// [`freshness_check`] once `now` and the high-water mark are available.
///
/// # Errors
/// - [`Error::Status`] for any non-`200`/non-`304` status.
/// - [`Error::Verify`] if the signed list does not verify against `pin`.
pub fn decide_outcome(
    status: u16,
    body: &str,
    etag: Option<String>,
    pin: Option<&str>,
) -> Result<FetchOutcome, Error> {
    match status {
        304 => Ok(FetchOutcome::NotModified),
        200 => {
            let verified = verify_signed_relay_list_any(body, &split_pins(pin))?;
            Ok(FetchOutcome::Updated {
                raw: body.to_owned(),
                etag,
                verified,
            })
        }
        other => Err(Error::Status(other)),
    }
}

/// Atomically replace the cache file: write to a sibling temp file then
/// `rename` over the target (atomic on the same filesystem), so a crash
/// mid-write can never leave a truncated `warren-relays.json` that the
/// next boot would fail to verify.
///
/// # Errors
/// [`Error::Io`] if the temp write or rename fails.
pub fn write_cache_atomic(cache_path: &Path, raw: &str) -> Result<(), Error> {
    crate::warren_artifact_refresh::write_cache_atomic(cache_path, raw)
        .map_err(|e| Error::Io(cache_path.display().to_string(), e))
}

/// Loads the bootstrap relay list at boot from the **newest** verifying
/// source among `<cache_dir>/warren-relays.json` (last fetched) and
/// `<resource_dir>/warren-relays.json` (baked into the build at CI time).
///
/// Mirrors upstream `parsed_relays::parse_relays_from_file`: a freshly
/// installed client that has never fetched still starts with the baked
/// list; a client that has fetched uses its newer cache. The newest file
/// is tried first; if it fails verification the other is tried, so a
/// corrupt cache cannot mask a good baked bootstrap. If neither verifies,
/// an empty list is returned (daemon boots, selection yields
/// `NoRelayMatch` until the first successful fetch).
///
/// Returns the verified list **including its `generation`** so the boot
/// path can seed the updater's anti-rollback high-water mark (a later
/// fetch with a lower generation is then rejected). **Expiry is NOT
/// enforced here**: the baked bootstrap is intentionally long-lived and
/// only seeds the first boot before the startup fetch refreshes; enforcing
/// it would brick a first-launch-offline client. Returns an empty
/// `VerifiedRelayList` (generation 0) when no source verifies.
#[must_use]
pub fn load_bootstrap(
    cache_dir: &Path,
    resource_dir: &Path,
    pin: Option<&str>,
) -> VerifiedRelayList {
    let mut candidates: Vec<(PathBuf, SystemTime)> = [cache_dir, resource_dir]
        .iter()
        .map(|d| d.join(WARREN_RELAYS_FILENAME))
        .filter_map(|p| {
            let mtime = std::fs::metadata(&p).and_then(|m| m.modified()).ok()?;
            Some((p, mtime))
        })
        .collect();
    // Newest first.
    candidates.sort_by_key(|(_, mtime)| std::cmp::Reverse(*mtime));

    for (path, _) in &candidates {
        match std::fs::read_to_string(path) {
            Ok(raw) => match verify_signed_relay_list_any(&raw, &split_pins(pin)) {
                Ok(verified) => {
                    log::info!(
                        "Warren bootstrap: loaded {} relays from {} (signature verified, generation {})",
                        verified.relays.len(),
                        path.display(),
                        verified.generation
                    );
                    return verified;
                }
                Err(e) => log::warn!(
                    "Warren bootstrap: {} failed verification ({e}); trying next source",
                    path.display()
                ),
            },
            Err(e) => log::warn!("Warren bootstrap: cannot read {} ({e})", path.display()),
        }
    }
    log::info!("Warren bootstrap: no verifying relay list found, starting empty");
    VerifiedRelayList {
        relays: WarrenRelayList::default(),
        generation: 0,
        signed_at: 0,
        expires_at: 0,
        server_pubkey_hex: String::new(),
    }
}

/// Handle to trigger an immediate refresh (e.g. the on-startup fetch, or
/// a manual `mullvad warren refresh`).
#[derive(Clone)]
pub struct WarrenRelayListUpdaterHandle {
    tx: mpsc::Sender<()>,
}

impl WarrenRelayListUpdaterHandle {
    /// Request an immediate refresh. Best-effort: a full channel (refresh
    /// already pending) or a shut-down updater is logged, not fatal.
    pub async fn update(&mut self) {
        if self.tx.send(()).await.is_err() {
            log::error!("Warren relay list updater is not running");
        }
    }
}

/// The periodic updater task. Owns the HTTP client, the ETag, the cache
/// path and the `on_update` sink that publishes a freshly verified list
/// to the rest of the daemon (selector hot-swap + GUI broadcast).
pub struct WarrenRelayListUpdater {
    api_url: String,
    cache_path: PathBuf,
    pinned_server_pubkey: Option<String>,
    etag: Option<String>,
    /// Highest `generation` ever accepted (anti-rollback high-water mark).
    /// Seeded from the bootstrap; a fetched list below this is rejected.
    highest_generation: u64,
    /// Whether the optional offline-admin roster feature is enabled.
    /// **Off by default** (gated by the daemon via `WARREN_ROSTER_ENABLED`).
    /// When `false`, the updater never fetches or enforces a roster: the
    /// online-signed `/v1/exits` list is published as-is. When `true`, it
    /// fetches `/v1/exits/roster`, verifies it against `roster_pin`, and
    /// drops any live-list exit the roster does not authorize.
    roster_enabled: bool,
    /// Pinned **offline admin** pubkey for verifying the roster. Only
    /// consulted when `roster_enabled`. Empty = TOFU (accepts any
    /// self-consistent roster signature) - operators enabling the feature
    /// should pin it via `WARREN_ADMIN_ROSTER_PUBKEY`.
    roster_pin: Option<String>,
    /// Latest verified offline-admin roster; the live list is filtered to
    /// roster-authorized exits before publishing. `None` = no roster yet
    /// (rollout transition → pass-through with a warning).
    roster: Option<VerifiedRoster>,
    /// Anti-rollback high-water mark for the roster.
    highest_roster_generation: u64,
    /// `ETag` of the last roster fetch.
    roster_etag: Option<String>,
    /// Whether a non-empty list was ever published. Once true, a
    /// subsequently *empty* result (transient registry drain, or a roster
    /// that dropped every live exit) is NOT published - we keep the
    /// last-good list rather than cut the user to zero exits (= no
    /// connectivity). Avoids a momentary backend blip / all-dropped roster
    /// bricking a working client.
    published_nonempty: bool,
    last_check: SystemTime,
    http: reqwest::Client,
    on_update: Box<dyn Fn(WarrenRelayList) + Send + 'static>,
    /// A retained clone of the command sender so the periodic refresh
    /// loop keeps running for the daemon's whole lifetime even if the
    /// returned [`WarrenRelayListUpdaterHandle`] is dropped (the channel
    /// only closes when this internal sender is also gone, i.e. never).
    /// Without it, dropping the handle would close the channel and end
    /// the task.
    _keepalive: mpsc::Sender<()>,
}

impl WarrenRelayListUpdater {
    /// Spawn the updater task. `on_update` is invoked with each newly
    /// verified list (the daemon wires it to swap the live selector and
    /// rebroadcast the GUI relay list). Returns a handle to force an
    /// immediate refresh - the caller should call
    /// [`WarrenRelayListUpdaterHandle::update`] once at boot for the
    /// on-startup fetch (mirrors upstream).
    #[expect(clippy::too_many_arguments)]
    pub fn spawn(
        api_url: String,
        cache_dir: &Path,
        pinned_server_pubkey: Option<String>,
        initial_etag: Option<String>,
        initial_generation: u64,
        roster_enabled: bool,
        roster_pin: Option<String>,
        initial_roster: Option<VerifiedRoster>,
        on_update: impl Fn(WarrenRelayList) + Send + 'static,
    ) -> WarrenRelayListUpdaterHandle {
        let (tx, rx) = mpsc::channel(1);
        let http = build_http_client(&api_url);
        let highest_roster_generation = initial_roster.as_ref().map_or(0, |r| r.generation);
        let updater = Self {
            api_url,
            cache_path: cache_dir.join(WARREN_RELAYS_FILENAME),
            pinned_server_pubkey,
            etag: initial_etag,
            highest_generation: initial_generation,
            roster_enabled,
            roster_pin,
            roster: initial_roster,
            highest_roster_generation,
            roster_etag: None,
            published_nonempty: false,
            last_check: UNIX_EPOCH,
            http,
            on_update: Box::new(on_update),
            _keepalive: tx.clone(),
        };
        tokio::spawn(updater.run(rx));
        WarrenRelayListUpdaterHandle { tx }
    }

    async fn run(mut self, mut events: mpsc::Receiver<()>) {
        // Roster enforcement is opt-in (off by default). When on, refresh
        // the roster before the first list so the very first publish is
        // filtered. A non-empty pin is required for real protection:
        // without it, any self-signed roster is trusted (TOFU), which an
        // operator enabling the feature almost certainly does not want.
        if self.roster_enabled {
            if split_pins(self.roster_pin.as_deref()).is_empty() {
                log::warn!(
                    "WARREN_ROSTER_ENABLED is set but no WARREN_ADMIN_ROSTER_PUBKEY pin is configured: any self-signed roster will be trusted (TOFU). Set the offline-admin pin or disable the roster."
                );
            }
            self.refresh_roster().await;
        }
        // On-startup fetch: refresh immediately so a daemon that just
        // booted on a stale baked bootstrap converges to the live list
        // without waiting a full check interval.
        let startup = Self::fetch_with_retry(
            self.http.clone(),
            self.api_url.clone(),
            self.etag.clone(),
            self.pinned_server_pubkey.clone(),
        )
        .await;
        self.consume(startup);
        self.last_check = SystemTime::now();

        let mut download = Box::pin(Fuse::terminated());
        loop {
            let next_check = tokio::time::sleep(UPDATE_CHECK_INTERVAL).fuse();
            futures::pin_mut!(next_check);
            futures::select! {
                _ = next_check => {
                    if download.is_terminated() && self.should_update() {
                        if self.roster_enabled {
                            self.refresh_roster().await;
                        }
                        download = Box::pin(
                            Self::fetch_with_retry(
                                self.http.clone(),
                                self.api_url.clone(),
                                self.etag.clone(),
                                self.pinned_server_pubkey.clone(),
                            )
                            .fuse(),
                        );
                        self.last_check = SystemTime::now();
                    }
                }
                outcome = download => self.consume(outcome),
                cmd = events.next() => {
                    if cmd.is_none() {
                        log::trace!("Warren relay list updater shutting down");
                        return;
                    }
                    if download.is_terminated() {
                        download = Box::pin(
                            Self::fetch_with_retry(
                                self.http.clone(),
                                self.api_url.clone(),
                                self.etag.clone(),
                                self.pinned_server_pubkey.clone(),
                            )
                            .fuse(),
                        );
                        self.last_check = SystemTime::now();
                    }
                }
            }
        }
    }

    /// True if the list is older than [`UPDATE_INTERVAL`]. A skewed clock
    /// (`Err`) forces a refresh to re-sync (mirrors upstream).
    fn should_update(&self) -> bool {
        SystemTime::now()
            .duration_since(self.last_check)
            .map(|d| d >= UPDATE_INTERVAL)
            .unwrap_or(true)
    }

    /// Best-effort refresh of the offline-admin roster (audit F1). On any
    /// failure (network, 404 = none published yet, bad signature, stale or
    /// rolled-back) the last-good roster is kept untouched - a roster
    /// refresh must never weaken enforcement.
    async fn refresh_roster(&mut self) {
        let url = format!("{}/v1/exits/roster", self.api_url.trim_end_matches('/'));
        match Self::fetch_roster_once(
            self.http.clone(),
            url,
            self.roster_etag.clone(),
            self.roster_pin.clone(),
        )
        .await
        {
            Ok(RosterFetch::NotModifiedOrAbsent) => {}
            Ok(RosterFetch::Updated { etag, verified }) => {
                let now = now_unix();
                if verified.is_expired(now) {
                    log::warn!("Warren roster is expired; keeping previous roster");
                    return;
                }
                if verified.generation < self.highest_roster_generation {
                    log::warn!(
                        "Warren roster rollback (generation {} < {}); keeping previous roster",
                        verified.generation,
                        self.highest_roster_generation
                    );
                    return;
                }
                self.roster_etag = etag;
                self.highest_roster_generation = verified.generation;
                log::info!(
                    "Warren roster updated: {} authorized exits (generation {})",
                    verified.entries.len(),
                    verified.generation
                );
                self.roster = Some(verified);
            }
            Err(e) => log::warn!("Warren roster refresh failed; keeping previous roster: {e}"),
        }
    }

    async fn fetch_roster_once(
        http: reqwest::Client,
        url: String,
        etag: Option<String>,
        pin: Option<String>,
    ) -> Result<RosterFetch, Error> {
        match conditional_get(&http, &url, etag).await? {
            // 304 (unchanged) and 404 (no roster published yet) both mean
            // "keep what we have"; the rollout pass-through handles None.
            FetchResponse::NotModified | FetchResponse::Status(404) => {
                Ok(RosterFetch::NotModifiedOrAbsent)
            }
            FetchResponse::Status(status) => Err(Error::Status(status)),
            FetchResponse::Body { body, etag } => {
                let verified = verify_roster_any(&body, &split_pins(pin.as_deref()))?;
                Ok(RosterFetch::Updated { etag, verified })
            }
        }
    }

    /// Builds the download+retry future from **owned** clones (no `&self`
    /// borrow), so the returned future can be stored in `download` across
    /// loop iterations without conflicting with the `&mut self` used by
    /// `consume`/`last_check`. Mirrors upstream's static `download_relay_list`.
    fn fetch_with_retry(
        http: reqwest::Client,
        api_url: String,
        etag: Option<String>,
        pinned_server_pubkey: Option<String>,
    ) -> impl std::future::Future<Output = Result<FetchOutcome, Error>> {
        let url = format!("{}/v1/exits", api_url.trim_end_matches('/'));
        let pin = pinned_server_pubkey;
        retry_future(
            move || Self::fetch_once(http.clone(), url.clone(), etag.clone(), pin.clone()),
            |res: &Result<FetchOutcome, Error>| {
                // Retry only transient transport/5xx errors. A signature
                // failure or a 4xx is not going to fix itself by retrying.
                matches!(res, Err(Error::Http(_)))
                    || matches!(res, Err(Error::Status(s)) if *s >= 500)
            },
            DOWNLOAD_RETRY_STRATEGY,
        )
    }

    async fn fetch_once(
        http: reqwest::Client,
        url: String,
        etag: Option<String>,
        pin: Option<String>,
    ) -> Result<FetchOutcome, Error> {
        match conditional_get(&http, &url, etag).await? {
            FetchResponse::NotModified => Ok(FetchOutcome::NotModified),
            FetchResponse::Status(status) => Err(Error::Status(status)),
            FetchResponse::Body { body, etag } => decide_outcome(200, &body, etag, pin.as_deref()),
        }
    }

    fn consume(&mut self, outcome: Result<FetchOutcome, Error>) {
        match outcome {
            Ok(FetchOutcome::NotModified) => log::debug!("Warren relay list is up-to-date"),
            Ok(FetchOutcome::Updated {
                raw,
                etag,
                verified,
            }) => {
                // Freshness gate (TUF rollback/freeze defense): a
                // signature-valid but stale (expired) or rolled-back
                // (lower generation) list is rejected here, keeping the
                // last-good list + cache untouched.
                if let Err(reason) = freshness_check(&verified, self.highest_generation, now_unix())
                {
                    log::warn!(
                        "Warren relay list rejected by freshness gate ({reason:?}); keeping previous list"
                    );
                    // Still update the ETag so we do not re-download the
                    // same rejected body every cycle; the high-water mark
                    // and the served list stay put.
                    self.etag = etag;
                    return;
                }
                // Accept: persist verbatim (already verified), advance the
                // anti-rollback high-water mark, then publish.
                if let Err(e) = write_cache_atomic(&self.cache_path, &raw) {
                    log::error!("Failed to write Warren relay cache: {e}");
                    // Still publish in-memory: a stale disk cache must not
                    // block a good live update.
                }
                self.etag = etag;
                self.highest_generation = verified.generation;
                // Drop any live-list exit the offline-admin roster does
                // not authorize before publishing to the selector.
                let enf = enforce_roster(verified.relays, self.roster.as_ref());
                if !enf.enforced {
                    if self.roster_enabled {
                        // Feature on but no roster fetched yet (rollout
                        // transition): pass-through, but flag it loudly.
                        log::warn!(
                            "Roster enforcement enabled but no roster available yet; serving the live exit list UNFILTERED (transient)"
                        );
                    } else {
                        // Feature off (default): pass-through is expected.
                        log::debug!(
                            "Roster feature disabled; serving the online-signed exit list as-is"
                        );
                    }
                } else if enf.dropped > 0 {
                    log::warn!(
                        "Roster dropped {} live-list exit(s) NOT authorized by the offline admin key (possible backend tampering)",
                        enf.dropped
                    );
                }
                // Availability guard: never replace a known-good
                // non-empty served list with an empty result (transient
                // registry drain, or a roster that dropped every exit).
                if should_publish(enf.list.relays().is_empty(), self.published_nonempty) {
                    if !enf.list.relays().is_empty() {
                        self.published_nonempty = true;
                    }
                    log::info!(
                        "Warren relay list updated: {} relays (generation {}, roster_enforced={})",
                        enf.list.len(),
                        verified.generation,
                        enf.enforced
                    );
                    (self.on_update)(enf.list);
                } else {
                    log::warn!(
                        "Warren relay list resolved to EMPTY but a non-empty list is already live; keeping last-good (transient drain or roster dropped all exits)"
                    );
                }
            }
            Err(e) => {
                // Keep last-good cache + in-memory list untouched.
                log::warn!("Warren relay list refresh failed, keeping previous list: {e}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use warren_discovery_core::warren_types::{ExitId, WarrenPubkey};
    use warren_discovery_core::{
        JsonEgress, JsonEndpoint, JsonListener, JsonLocation, JsonNode, sign_relay_list,
    };

    /// Far-future expiry so default fixtures are never "expired".
    const FAR_FUTURE: u64 = 4_000_000_000;

    #[test]
    fn pin_target_only_pins_default_host_when_ip_set() {
        use mullvad_api_constants::API_HOST_DEFAULT;
        use std::net::{IpAddr, Ipv4Addr, SocketAddr};
        let ip = IpAddr::V4(Ipv4Addr::new(203, 0, 113, 7));

        // The default host follows the compiled product environment; build
        // the tested URLs from it so the contract holds for every env.
        let default_url = format!("https://{API_HOST_DEFAULT}");

        // No pinned IP → never pins (the shipped default, unchanged behaviour).
        assert_eq!(pin_target(&default_url, None), None);

        // Pinned IP + default host → pins to <ip>:443.
        assert_eq!(
            pin_target(&format!("{default_url}/v1/exits"), Some(ip)),
            Some(SocketAddr::new(ip, 443))
        );
        // Host match is case-insensitive.
        assert_eq!(
            pin_target(
                &format!("https://{}", API_HOST_DEFAULT.to_uppercase()),
                Some(ip)
            ),
            Some(SocketAddr::new(ip, 443))
        );

        // Pinned IP but a DIFFERENT host (operator override) → must NOT pin.
        assert_eq!(pin_target("https://staging.example.org", Some(ip)), None);
        // Another environment's host counts as different for this build:
        // a beta daemon must never pin prod's host, and vice versa.
        if API_HOST_DEFAULT != "api.warrenbrowse.com" {
            assert_eq!(pin_target("https://api.warrenbrowse.com", Some(ip)), None);
        } else {
            assert_eq!(
                pin_target("https://api.beta.warrenbrowse.com", Some(ip)),
                None
            );
        }
        // Garbage URL → no pin (graceful).
        assert_eq!(pin_target("not a url", Some(ip)), None);
    }

    fn signed_body(server_key: &SigningKey, seed: u8) -> String {
        signed_body_full(server_key, seed, 1, FAR_FUTURE)
    }

    fn signed_body_full(
        server_key: &SigningKey,
        seed: u8,
        generation: u64,
        expires_at: u64,
    ) -> String {
        let relay_pubkey_hex = hex::encode(WarrenPubkey::from_bytes([seed; 32]).as_bytes());
        let signed = sign_relay_list(
            vec![JsonNode {
                id: relay_pubkey_hex,
                exit_id: ExitId::from_bytes([seed; 16]),
                cover_domain: None,
                tcp_fallback: None,
                last_seen_unix: None,
                stale: None,
                name: None,
                provider: None,
                virt: None,
                asn: None,
                attestation_hex: None,
                relay_descriptor: None,
                exit_descriptor: None,
                edge_cert_sha256: None,
                location: JsonLocation {
                    country: "se".to_owned(),
                    city: "Stockholm".to_owned(),
                },
                weight: 100,
                active: true,
                egress: JsonEgress {
                    ipv4: true,
                    ipv6: false,
                },
                endpoints: vec![JsonEndpoint {
                    addr: "198.51.100.1".to_owned(),
                    family: "ipv4".to_owned(),
                    listeners: vec![JsonListener {
                        port: 51820,
                        transport: "quic".to_owned(),
                        alpn: "h3".to_owned(),
                    }],
                }],
                port_forward: None,
            }],
            server_key,
            generation,
            1_700_000_000,
            expires_at,
        );
        serde_json::to_string(&signed).expect("serialize signed v4")
    }

    fn verified_of(server_key: &SigningKey, generation: u64, expires_at: u64) -> VerifiedRelayList {
        verify_signed_relay_list_any(
            &signed_body_full(server_key, 5, generation, expires_at),
            &[],
        )
        .expect("must verify")
    }

    fn pubkey_hex(key: &SigningKey) -> String {
        hex::encode(key.verifying_key().to_bytes())
    }

    #[test]
    fn decide_outcome_304_is_not_modified() {
        let out = decide_outcome(304, "", Some("W/\"7\"".into()), None).expect("304 ok");
        assert!(matches!(out, FetchOutcome::NotModified));
    }

    #[test]
    fn decide_outcome_200_valid_signed_yields_updated_list() {
        let key = SigningKey::from_bytes(&[0xab; 32]);
        let body = signed_body(&key, 5);
        let out = decide_outcome(200, &body, Some("W/\"9\"".into()), Some(&pubkey_hex(&key)))
            .expect("valid signed body must verify");
        match out {
            FetchOutcome::Updated {
                verified,
                etag,
                raw,
            } => {
                assert_eq!(verified.relays.len(), 1, "one relay parsed");
                assert_eq!(
                    verified.generation, 1,
                    "generation surfaced for freshness gate"
                );
                assert_eq!(etag.as_deref(), Some("W/\"9\""));
                assert_eq!(raw, body, "raw body preserved verbatim for caching");
            }
            FetchOutcome::NotModified => panic!("expected Updated"),
        }
    }

    #[test]
    fn decide_outcome_rejects_wrong_pinned_pubkey() {
        // A list correctly self-signed by an ATTACKER key must be refused
        // when we pin the legitimate server key (anti key-swap).
        let attacker = SigningKey::from_bytes(&[0x11; 32]);
        let legit = SigningKey::from_bytes(&[0xab; 32]);
        let body = signed_body(&attacker, 5);
        let err = decide_outcome(200, &body, None, Some(&pubkey_hex(&legit)))
            .expect_err("pinned mismatch must error");
        assert!(matches!(err, Error::Verify(_)));
    }

    #[test]
    fn decide_outcome_rejects_tampered_body() {
        let key = SigningKey::from_bytes(&[0xab; 32]);
        let mut body = signed_body(&key, 5);
        // Flip a byte inside the signed payload (port digit) without
        // re-signing -> signature must fail.
        body = body.replace("51820", "59999");
        let err = decide_outcome(200, &body, None, Some(&pubkey_hex(&key)))
            .expect_err("tampered body must error");
        assert!(matches!(err, Error::Verify(_)));
    }

    #[test]
    fn decide_outcome_non_2xx_is_status_error() {
        let err = decide_outcome(503, "upstream down", None, None).expect_err("5xx errors");
        assert!(matches!(err, Error::Status(503)));
    }

    // write_cache_atomic behavior tests moved with the shared
    // implementation to `warren_artifact_refresh`.

    #[test]
    fn load_bootstrap_empty_when_no_sources() {
        let dir = isolated_tempdir();
        let other = isolated_tempdir();
        let list = load_bootstrap(&dir, &other, None);
        assert_eq!(list.relays.len(), 0);
        assert_eq!(list.generation, 0, "no source -> generation seed 0");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&other);
    }

    #[test]
    fn load_bootstrap_prefers_newer_cache_over_baked_resource() {
        let key = SigningKey::from_bytes(&[0xab; 32]);
        let cache = isolated_tempdir();
        let resource = isolated_tempdir();
        // Resource (baked) has relay seed 7; cache (fetched later) seed 9.
        std::fs::write(resource.join(WARREN_RELAYS_FILENAME), signed_body(&key, 7)).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(cache.join(WARREN_RELAYS_FILENAME), signed_body(&key, 9)).unwrap();

        let list = load_bootstrap(&cache, &resource, Some(&pubkey_hex(&key)));
        assert_eq!(list.relays.len(), 1);
        let seed9 = WarrenPubkey::from_bytes([9; 32]);
        assert!(
            list.relays
                .relays()
                .iter()
                .any(|r| r.endpoint_id() == seed9),
            "newer cache (seed 9) must win over baked resource (seed 7)"
        );
        let _ = std::fs::remove_dir_all(&cache);
        let _ = std::fs::remove_dir_all(&resource);
    }

    #[test]
    fn load_bootstrap_falls_back_to_resource_when_cache_corrupt() {
        let key = SigningKey::from_bytes(&[0xab; 32]);
        let cache = isolated_tempdir();
        let resource = isolated_tempdir();
        // Baked resource is valid; cache is newer but corrupt -> must fall
        // back to the verifying baked list rather than ending up empty.
        std::fs::write(resource.join(WARREN_RELAYS_FILENAME), signed_body(&key, 7)).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(cache.join(WARREN_RELAYS_FILENAME), "{ not valid").unwrap();

        let list = load_bootstrap(&cache, &resource, Some(&pubkey_hex(&key)));
        assert_eq!(
            list.relays.len(),
            1,
            "corrupt cache must fall back to baked resource"
        );
        let _ = std::fs::remove_dir_all(&cache);
        let _ = std::fs::remove_dir_all(&resource);
    }

    #[test]
    fn freshness_accepts_fresh_higher_generation() {
        let key = SigningKey::from_bytes(&[0xab; 32]);
        let v = verified_of(&key, 10, FAR_FUTURE);
        // generation 10 > high-water 5, not expired -> accepted.
        assert!(freshness_check(&v, 5, 1_700_000_500).is_ok());
    }

    #[test]
    fn freshness_accepts_equal_generation_idempotent_refetch() {
        // Re-fetching the SAME current generation is fine (e.g. after a
        // restart): equality is not a rollback.
        let key = SigningKey::from_bytes(&[0xab; 32]);
        let v = verified_of(&key, 7, FAR_FUTURE);
        assert!(freshness_check(&v, 7, 1_700_000_500).is_ok());
    }

    #[test]
    fn freshness_rejects_lower_generation_rollback() {
        // Rollback attack: server (or CDN/attacker) serves an older,
        // still-signature-valid list. Must be rejected to keep a revoked
        // exit from coming back / a drained one from reappearing.
        let key = SigningKey::from_bytes(&[0xab; 32]);
        let v = verified_of(&key, 3, FAR_FUTURE);
        assert_eq!(
            freshness_check(&v, 9, 1_700_000_500),
            Err(FreshnessReject::Rollback { got: 3, have: 9 })
        );
    }

    #[test]
    fn freshness_rejects_expired_replay() {
        // Freeze/replay attack: a captured-then-replayed list whose signed
        // expiry has passed must be rejected even though its signature and
        // generation are otherwise acceptable.
        let key = SigningKey::from_bytes(&[0xab; 32]);
        let v = verified_of(&key, 100, 1_700_086_400);
        let now = 1_700_090_000; // past expiry
        assert_eq!(
            freshness_check(&v, 0, now),
            Err(FreshnessReject::Expired {
                expires_at: 1_700_086_400,
                now
            })
        );
    }

    #[test]
    fn freshness_expiry_checked_before_rollback() {
        // An expired AND rolled-back list reports Expired first (order is
        // deterministic; both are rejections, the precedence just keeps
        // the log message stable).
        let key = SigningKey::from_bytes(&[0xab; 32]);
        let v = verified_of(&key, 1, 1_700_086_400);
        assert!(matches!(
            freshness_check(&v, 9, 1_700_090_000),
            Err(FreshnessReject::Expired { .. })
        ));
    }

    fn roster_with(admin: &SigningKey, seed: u8) -> VerifiedRoster {
        let entry = warren_discovery_core::RosterEntry {
            endpoint_id: hex::encode(WarrenPubkey::from_bytes([seed; 32]).as_bytes()),
            exit_id: ExitId::from_bytes([seed; 16]),
            country: "se".to_owned(),
            city: "Stockholm".to_owned(),
        };
        let signed = warren_discovery_core::sign_roster(vec![entry], admin, 1, 1_000, FAR_FUTURE);
        verify_roster_any(&serde_json::to_string(&signed).unwrap(), &[]).expect("roster verify")
    }

    #[test]
    fn enforce_roster_passes_through_when_no_roster() {
        // Rollout transition: with no roster yet, the list passes through
        // unfiltered (enforced=false) so a pre-roster client is not bricked.
        let key = SigningKey::from_bytes(&[0xab; 32]);
        let list = verify_signed_relay_list_any(&signed_body(&key, 5), &[])
            .unwrap()
            .relays;
        let n = list.relays().len();
        let enf = enforce_roster(list, None);
        assert!(!enf.enforced, "no roster -> not enforced");
        assert_eq!(enf.dropped, 0);
        assert_eq!(enf.list.relays().len(), n, "list passes through unchanged");
    }

    #[test]
    fn enforce_roster_keeps_authorized_exit() {
        let key = SigningKey::from_bytes(&[0xab; 32]);
        let admin = SigningKey::from_bytes(&[0x42; 32]);
        let list = verify_signed_relay_list_any(&signed_body(&key, 5), &[])
            .unwrap()
            .relays;
        let roster = roster_with(&admin, 5); // authorizes the seed-5 exit
        let enf = enforce_roster(list, Some(&roster));
        assert!(enf.enforced);
        assert_eq!(enf.dropped, 0);
        assert_eq!(enf.list.relays().len(), 1, "authorized exit kept");
    }

    #[test]
    fn enforce_roster_drops_exit_absent_from_roster() {
        // The core property at the client edge: a live-list exit that
        // the offline roster does not authorize (e.g. injected by a
        // compromised backend) is dropped before reaching the selector.
        let key = SigningKey::from_bytes(&[0xab; 32]);
        let admin = SigningKey::from_bytes(&[0x42; 32]);
        let list = verify_signed_relay_list_any(&signed_body(&key, 5), &[])
            .unwrap()
            .relays;
        let roster = roster_with(&admin, 6); // authorizes a DIFFERENT exit
        let enf = enforce_roster(list, Some(&roster));
        assert!(enf.enforced);
        assert_eq!(enf.dropped, 1, "unauthorized exit dropped");
        assert_eq!(enf.list.relays().len(), 0);
    }

    #[test]
    fn should_publish_keeps_last_good_only_on_empty_after_nonempty() {
        assert!(should_publish(false, false), "non-empty fresh -> publish");
        assert!(should_publish(false, true), "non-empty -> publish");
        assert!(
            should_publish(true, false),
            "first-ever empty -> publish (UI shows 'no exits yet')"
        );
        assert!(
            !should_publish(true, true),
            "empty after a non-empty was served -> KEEP last-good (no zero-exit cutover)"
        );
    }

    fn isolated_tempdir() -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("warren-relay-updater-{pid}-{nanos}-{n}"));
        std::fs::create_dir_all(&dir).expect("create tempdir");
        dir
    }
}
