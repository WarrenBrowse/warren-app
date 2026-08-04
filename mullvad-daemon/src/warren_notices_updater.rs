//! Periodic refresher for the server-signed broadcast notices
//! (`GET /v1/notices`).
//!
//! Third instance of the signed-artifact refresh pattern shared with the
//! relay-list and multi-hop-directory updaters (ETag conditional GET,
//! post-wake fast retry, verify before use), with two deliberate
//! differences that both come from what a notice IS:
//!
//! - **No on-disk cache.** A notice is a live statement by the operator,
//!   not state the client needs to boot with. Persisting one across
//!   restarts would only create a way for an erased message to come back
//!   on a client that cannot reach the API.
//! - **Nothing is displayed unless it verified just now.** The envelope
//!   carries a short signed expiry; once it lapses the banner drops. So a
//!   blocked or hostile network can suppress a notice (it could suppress
//!   the whole API anyway) but cannot freeze one on screen.
//!
//! The version filter and the per-notice TTL are applied here rather than
//! in the UI: the daemon knows the app version, and the renderer should
//! receive exactly what it must display.

use std::time::Duration;

use warren_discovery_core::{NoticesError, VerifiedNotices, verify_signed_notices_any};

use crate::warren_artifact_refresh::{
    FETCH_TIMEOUT, FetchResponse, TransportRetryBackoff, conditional_get, now_unix, split_pins,
};

/// How often the notices are re-fetched. This is the delay an operator
/// waits for a publication or an erasure to reach a running client, so it
/// is much shorter than the relay-list cadence; the request is a
/// conditional GET, so an unchanged set costs one header round trip.
///
/// It is also a poll the API can see, which is why it is not shorter: the
/// request carries no account identity (the endpoint is public and
/// unauthenticated), but the cadence is still traffic, and erasing within
/// minutes is what the feature needs, not within seconds.
const CHECK_INTERVAL: Duration = Duration::from_secs(5 * 60);

/// Fast retry window after a transport failure. A client that just woke up
/// or just regained a network should not sit up to a full interval showing
/// nothing (or showing a notice whose envelope is about to lapse).
const RETRY_MIN: Duration = Duration::from_secs(20);
/// Ceiling for the fast retry, kept under [`CHECK_INTERVAL`] so a long
/// outage degrades into the normal cadence rather than into silence.
const RETRY_MAX: Duration = Duration::from_secs(240);

/// One notice as handed to the UI: already filtered for expiry and client
/// version, so the renderer displays the list verbatim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayNotice {
    /// Server-assigned id, forwarded so the UI can keep per-notice state.
    pub id: String,
    /// Operator text, rendered as plain text (never as markup).
    pub message: String,
    /// Severity, driving the banner's indicator colour.
    pub level: NoticeLevel,
}

/// Severity of a [`DisplayNotice`], mirroring the wire enum without
/// leaking the contract type into the management interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoticeLevel {
    /// Informational.
    Info,
    /// Warning.
    Warning,
    /// Critical.
    Error,
}

impl From<warren_discovery_core::NoticeLevel> for NoticeLevel {
    fn from(level: warren_discovery_core::NoticeLevel) -> Self {
        use warren_discovery_core::NoticeLevel as Wire;
        match level {
            Wire::Info => Self::Info,
            Wire::Warning => Self::Warning,
            Wire::Error => Self::Error,
        }
    }
}

/// Build the HTTP client. Same posture as the relay-list fetcher: the API
/// host resolves from the daemon's address cache ([`crate::warren_api_dns`]),
/// and when an API IP is pinned and the URL targets the default host, resolve
/// without DNS and omit SNI.
fn build_http_client(api_url: &str) -> reqwest::Client {
    use mullvad_api_constants::{API_HOST_DEFAULT, API_PINNED_IP, API_PORT_DEFAULT};

    let mut builder =
        crate::warren_api_dns::with_api_resolver(reqwest::Client::builder().timeout(FETCH_TIMEOUT));
    if let Some(ip) = API_PINNED_IP
        && reqwest::Url::parse(api_url)
            .ok()
            .and_then(|u| {
                u.host_str()
                    .map(|h| h.eq_ignore_ascii_case(API_HOST_DEFAULT))
            })
            .unwrap_or(false)
    {
        builder = builder
            .resolve(
                API_HOST_DEFAULT,
                std::net::SocketAddr::new(ip, API_PORT_DEFAULT),
            )
            .tls_sni(false);
    }
    builder
        .build()
        .expect("reqwest client build failed: invalid TLS backend configuration")
}

/// The periodic notices task.
pub struct WarrenNoticesUpdater {
    api_url: String,
    /// Pinned server pubkey set (comma-separated for rotation). Empty
    /// means TOFU, which the daemon never configures in a real build.
    pinned_server_pubkey: Option<String>,
    /// This app's own version, used to honour a notice's declared version
    /// range. `None` withholds every range-restricted notice.
    client_version: Option<String>,
    etag: Option<String>,
    /// Highest generation ever accepted (anti-rollback high-water mark).
    highest_generation: u64,
    /// Last verified envelope, kept in memory only. Re-published on a
    /// `304` so its own expiry rules keep being applied while the set is
    /// unchanged.
    last: Option<VerifiedNotices>,
    http: reqwest::Client,
    on_update: Box<dyn Fn(Vec<DisplayNotice>) + Send>,
}

impl WarrenNoticesUpdater {
    /// Spawn the updater. `on_update` receives the notices to display,
    /// including the empty vector when the last one is erased or lapses,
    /// so the UI clears its banner from the same signal that raised it.
    pub fn spawn(
        api_url: String,
        pinned_server_pubkey: Option<String>,
        client_version: Option<String>,
        on_update: impl Fn(Vec<DisplayNotice>) + Send + 'static,
    ) {
        let http = build_http_client(&api_url);
        let updater = Self {
            api_url,
            pinned_server_pubkey,
            client_version,
            etag: None,
            highest_generation: 0,
            last: None,
            http,
            on_update: Box::new(on_update),
        };
        tokio::spawn(updater.run());
    }

    async fn run(mut self) {
        let mut retry = TransportRetryBackoff::new(RETRY_MIN, RETRY_MAX);
        loop {
            match self.refresh_once().await {
                RefreshOutcome::Ok | RefreshOutcome::Rejected => retry.clear(),
                RefreshOutcome::TransportFailure => {
                    retry.on_transport_failure();
                }
            }
            let delay = retry.delay().unwrap_or(CHECK_INTERVAL);
            tokio::time::sleep(delay).await;
        }
    }

    /// One conditional fetch plus verification and publication.
    async fn refresh_once(&mut self) -> RefreshOutcome {
        let url = format!("{}/v1/notices", self.api_url.trim_end_matches('/'));
        let response = match conditional_get(&self.http, &url, self.etag.clone()).await {
            Ok(r) => r,
            Err(e) => {
                log::debug!("Warren notices fetch failed: {e}");
                return RefreshOutcome::TransportFailure;
            }
        };
        match response {
            // A 304 says the *set* has not changed, so the last publish
            // still stands. The envelope the client verified earlier is
            // what bounds how long it may be displayed, and it is
            // re-published below so the UI re-applies its expiry.
            FetchResponse::NotModified => {
                self.republish_last();
                RefreshOutcome::Ok
            }
            FetchResponse::Status(status) => {
                log::debug!("Warren notices endpoint returned {status}");
                RefreshOutcome::Rejected
            }
            FetchResponse::Body { body, etag } => {
                let pins = split_pins(self.pinned_server_pubkey.as_deref());
                match verify_signed_notices_any(&body, &pins) {
                    Ok(verified) => {
                        if is_rollback(verified.generation, self.highest_generation) {
                            // Rollback: an older envelope, replayed. It
                            // could hold a notice the operator erased.
                            log::warn!(
                                "Warren notices generation went backwards ({} < {}); ignoring",
                                verified.generation,
                                self.highest_generation
                            );
                            return RefreshOutcome::Rejected;
                        }
                        self.highest_generation = verified.generation;
                        self.etag = etag;
                        self.publish(&verified);
                        self.last = Some(verified);
                        RefreshOutcome::Ok
                    }
                    Err(e) => {
                        // Never fall back to the body: an unverified
                        // envelope is exactly the phishing case the
                        // signature exists to stop.
                        log::error!("Warren notices verification failed: {}", describe(&e));
                        RefreshOutcome::Rejected
                    }
                }
            }
        }
    }

    /// Re-applies the last verified envelope's own expiry rules. Called on
    /// `304`, so a set that stops being displayable while unchanged (a
    /// per-notice TTL elapses, or the envelope lapses) still clears.
    fn republish_last(&self) {
        if let Some(verified) = &self.last {
            self.publish(verified);
        }
    }

    fn publish(&self, verified: &VerifiedNotices) {
        let now = now_unix();
        let notices = verified
            .active_for(now, self.client_version.as_deref())
            .into_iter()
            .map(|n| DisplayNotice {
                id: n.id.clone(),
                message: n.message.clone(),
                level: n.level.into(),
            })
            .collect();
        (self.on_update)(notices);
    }
}

/// Outcome of one refresh, used only to drive the retry policy.
enum RefreshOutcome {
    /// Fetched (or confirmed unchanged) and published.
    Ok,
    /// Reached the server but did not accept the answer. A fast retry
    /// would not help, so this falls back to the normal cadence.
    Rejected,
    /// Never reached the server.
    TransportFailure,
}

/// True when an incoming envelope is older than the highest one already
/// trusted, i.e. a replay that could resurrect an erased notice. Equal
/// generations are accepted: the server re-signs the same set
/// periodically, and refusing that would freeze a client on a stale
/// envelope that is about to expire.
fn is_rollback(incoming: u64, highest: u64) -> bool {
    incoming < highest
}

/// Redacted one-line reason for a verification failure. The message must
/// never carry the pubkey or the body.
fn describe(e: &NoticesError) -> &'static str {
    match e {
        NoticesError::Json(_) => "malformed envelope",
        NoticesError::UnsupportedVersion { .. } => "unsupported envelope version",
        NoticesError::ServerPubkeyMismatch { .. } => "server pubkey is not the pinned one",
        NoticesError::InvalidHex | NoticesError::PubkeyNotOnCurve => "malformed key or signature",
        NoticesError::BadSignature => "bad signature",
        NoticesError::InputTooLarge => "envelope too large",
        _ => "verification failed",
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use warren_discovery_core::{Notice, NoticeLevel as WireLevel, VerifiedNotices};

    use super::*;

    fn notice(id: &str, message: &str, level: WireLevel, expires_at: Option<u64>) -> Notice {
        Notice {
            id: id.to_owned(),
            message: message.to_owned(),
            level,
            min_client_version: None,
            max_client_version: None,
            expires_at,
        }
    }

    /// An updater whose `on_update` records what it published, with no
    /// HTTP client involvement.
    fn recording_updater(
        client_version: Option<&str>,
    ) -> (WarrenNoticesUpdater, Arc<Mutex<Vec<Vec<DisplayNotice>>>>) {
        let published = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&published);
        let updater = WarrenNoticesUpdater {
            api_url: "https://example.invalid".to_owned(),
            pinned_server_pubkey: None,
            client_version: client_version.map(str::to_owned),
            etag: None,
            highest_generation: 0,
            last: None,
            http: reqwest::Client::new(),
            on_update: Box::new(move |notices| {
                sink.lock().expect("test sink lock").push(notices);
            }),
        };
        (updater, published)
    }

    /// An envelope whose expiry is expressed relative to the real clock,
    /// because `publish` reads the real clock (that is the behaviour under
    /// test: a lapsed envelope must publish nothing).
    fn envelope(notices: Vec<Notice>, valid_for: i64) -> VerifiedNotices {
        let now = now_unix();
        VerifiedNotices {
            notices,
            generation: 1,
            signed_at: now,
            expires_at: now.saturating_add_signed(valid_for),
        }
    }

    #[test]
    fn publishes_the_operator_message_and_its_level() {
        let (updater, published) = recording_updater(None);
        let verified = envelope(
            vec![notice("a1", "exit outage in NL", WireLevel::Error, None)],
            3_600,
        );

        updater.publish(&verified);

        let batches = published.lock().expect("sink");
        assert_eq!(
            batches.as_slice(),
            &[vec![DisplayNotice {
                id: "a1".to_owned(),
                message: "exit outage in NL".to_owned(),
                level: NoticeLevel::Error,
            }]],
            "the UI must receive the message verbatim with its severity"
        );
    }

    #[test]
    fn publishes_an_empty_batch_once_the_envelope_has_lapsed() {
        // Anti-freeze: this is what clears the banner on a client whose
        // refresh is being blocked, without any renderer-side timer.
        let (updater, published) = recording_updater(None);
        let verified = envelope(vec![notice("a1", "stale", WireLevel::Info, None)], -1);

        updater.publish(&verified);

        assert_eq!(
            published.lock().expect("sink").as_slice(),
            &[Vec::<DisplayNotice>::new()],
            "an expired envelope must publish the empty set, not nothing at all"
        );
    }

    #[test]
    fn withholds_a_version_targeted_notice_from_an_out_of_range_client() {
        let (updater, published) = recording_updater(Some("1.9.0"));
        let mut targeted = notice("a1", "for 1.11 and up", WireLevel::Warning, None);
        targeted.min_client_version = Some("1.11.0".to_owned());
        let verified = envelope(vec![targeted], 3_600);

        updater.publish(&verified);

        assert_eq!(
            published.lock().expect("sink").as_slice(),
            &[Vec::<DisplayNotice>::new()],
            "the daemon applies the version range, so the UI never has to"
        );
    }

    #[test]
    fn a_lower_generation_is_a_rollback_and_an_equal_one_is_not() {
        assert!(
            is_rollback(4, 5),
            "an older envelope could carry an erased notice"
        );
        assert!(
            !is_rollback(5, 5),
            "the server re-signs the same set periodically; refusing that would \
             strand the client on an envelope that is about to expire"
        );
        assert!(!is_rollback(6, 5));
    }
}
