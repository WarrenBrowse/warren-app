//! Periodic refresher for the server-signed forum activity digest
//! (`GET /v1/forum/digest`).
//!
//! Fourth instance of the signed-artifact refresh pattern shared with the
//! relay-list, multi-hop-directory and notices updaters, and it inherits
//! the notices posture for the same reasons: no on-disk cache, and
//! nothing surfaced unless it verified against the pinned server key.
//!
//! What makes this one different is what it does NOT do. The document is
//! one anonymous array of unread counts, identical for every client, so
//! this fetch says nothing about the user: it carries no account, and its
//! cadence cannot be tied to one. The slot that turns the array into a
//! badge is known only to the renderer, which holds it beside the forum
//! handle in its own sealed store, so the counts are published here
//! verbatim and indexed there.
//!
//! Freshness is applied on every cycle, not only when the document
//! changes: a server that stops answering must let the badge lapse, and
//! an envelope whose expiry passes while nothing changes would otherwise
//! stay on screen until the next edit.

use std::time::Duration;

use warren_discovery_core::{ForumDigestError, VerifiedForumDigest, verify_forum_digest_any};

use crate::warren_artifact_refresh::{
    FETCH_TIMEOUT, FetchResponse, TransportRetryBackoff, conditional_get, now_unix, split_pins,
};

/// How often the digest is re-fetched: how long a reply takes to raise a
/// badge, and how long a badge cleared elsewhere takes to drop here.
///
/// Shorter than the notices cadence it used to share, because this one
/// carries a number the user checks against the forum and expects to
/// agree. The cost is bounded by design: the request is a conditional GET
/// on a document identical for every client, so a quiet forum answers it
/// with a 304 and a few hundred bytes. Anything the user does in the app
/// itself corrects the count immediately without waiting for this at all.
const CHECK_INTERVAL: Duration = Duration::from_secs(60);

/// Fast retry window after a transport failure, so a client that just
/// woke or just regained a network does not sit a full interval with a
/// badge it can no longer justify.
const RETRY_MIN: Duration = Duration::from_secs(20);
/// Ceiling for the fast retry, kept under [`CHECK_INTERVAL`].
const RETRY_MAX: Duration = Duration::from_secs(45);

/// The periodic forum-digest task.
pub struct WarrenForumDigestUpdater {
    api_url: String,
    /// Pinned server pubkey set (comma-separated for rotation). Empty
    /// means TOFU, which the daemon never configures in a real build.
    pinned_server_pubkey: Option<String>,
    etag: Option<String>,
    /// Highest generation ever accepted (anti-rollback high-water mark).
    highest_generation: u64,
    /// Last verified document, in memory only.
    last: Option<VerifiedForumDigest>,
    http: reqwest::Client,
    on_update: Box<dyn Fn(Option<String>) + Send>,
}

impl WarrenForumDigestUpdater {
    /// Spawn the updater. `on_update` receives the verified counts while
    /// a fresh document is held, and `None` the moment there is not one,
    /// so the UI clears its badge from the same signal that raised it.
    pub fn spawn(
        api_url: String,
        pinned_server_pubkey: Option<String>,
        on_update: impl Fn(Option<String>) + Send + 'static,
    ) {
        let http = build_http_client(&api_url);
        let updater = Self {
            api_url,
            pinned_server_pubkey,
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
            // Re-apply freshness whatever happened. A server answering 503
            // (or nothing at all) must let the badge lapse on its own
            // expiry, and the publish is idempotent, so this costs nothing
            // when the document already stands.
            self.publish();
            let delay = retry.delay().unwrap_or(CHECK_INTERVAL);
            tokio::time::sleep(delay).await;
        }
    }

    /// One conditional fetch plus verification.
    async fn refresh_once(&mut self) -> RefreshOutcome {
        let url = format!("{}/v1/forum/digest", self.api_url.trim_end_matches('/'));
        let response = match conditional_get(&self.http, &url, self.etag.clone()).await {
            Ok(r) => r,
            Err(e) => {
                log::debug!("Warren forum digest fetch failed: {e}");
                return RefreshOutcome::TransportFailure;
            }
        };
        match response {
            FetchResponse::NotModified => RefreshOutcome::Ok,
            FetchResponse::Status(status) => {
                log::debug!("Warren forum digest endpoint returned {status}");
                RefreshOutcome::Rejected
            }
            FetchResponse::Body { body, etag } => {
                let pins = split_pins(self.pinned_server_pubkey.as_deref());
                match verify_forum_digest_any(&body, &pins) {
                    Ok(verified) => {
                        if verified.generation < self.highest_generation {
                            // A replayed older document could put back a
                            // badge the reader has already cleared.
                            log::warn!(
                                "Warren forum digest generation went backwards ({} < {}); ignoring",
                                verified.generation,
                                self.highest_generation
                            );
                            return RefreshOutcome::Rejected;
                        }
                        self.highest_generation = verified.generation;
                        self.etag = etag;
                        self.last = Some(verified);
                        RefreshOutcome::Ok
                    }
                    Err(e) => {
                        // Never fall back to the body: a badge is an
                        // invitation to click, which is exactly what the
                        // signature exists to stop anyone else raising.
                        log::error!("Warren forum digest verification failed: {}", describe(&e));
                        RefreshOutcome::Rejected
                    }
                }
            }
        }
    }

    /// Publishes the counts while the held document is fresh, `None`
    /// otherwise.
    fn publish(&self) {
        let counts = self
            .last
            .as_ref()
            .filter(|verified| !verified.is_expired(now_unix()))
            .map(|verified| verified.counts_hex().to_owned());
        (self.on_update)(counts);
    }
}

/// Build the HTTP client. Same posture as the notices fetcher: the API
/// host resolves from the daemon's address cache, and when an API IP is
/// pinned and the URL targets the default host, resolve without DNS and
/// omit SNI.
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

/// Outcome of one refresh, used only to drive the retry policy.
enum RefreshOutcome {
    /// Fetched (or confirmed unchanged).
    Ok,
    /// Reached the server but did not accept the answer.
    Rejected,
    /// Never reached the server.
    TransportFailure,
}

/// Redacted one-line reason for a verification failure. Never carries the
/// pubkey or the body.
fn describe(e: &ForumDigestError) -> &'static str {
    match e {
        ForumDigestError::Json(_) => "malformed document",
        ForumDigestError::UnsupportedVersion { .. } => "unsupported document version",
        ForumDigestError::ServerPubkeyMismatch { .. } => "server pubkey is not the pinned one",
        ForumDigestError::InvalidHex | ForumDigestError::PubkeyNotOnCurve => {
            "malformed key or signature"
        }
        ForumDigestError::BadSignature => "bad signature",
        ForumDigestError::InvalidCounts => "malformed counts",
        ForumDigestError::InputTooLarge => "document too large",
        _ => "verification failed",
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use ed25519_dalek::SigningKey;
    use warren_discovery_core::{pack_unread_counts, sign_forum_digest};

    use super::*;

    fn server_key() -> SigningKey {
        SigningKey::from_bytes(&[0x44; 32])
    }

    fn pin() -> String {
        hex::encode(server_key().verifying_key().as_bytes())
    }

    fn signed_json(counts: &[u32], generation: u64, expires_at: u64) -> String {
        let signed = sign_forum_digest(
            pack_unread_counts(counts),
            &server_key(),
            generation,
            now_unix(),
            expires_at,
        );
        serde_json::to_string(&signed).expect("serialize")
    }

    /// An updater with no network, driven by handing it documents the way
    /// `refresh_once` would after verification.
    fn updater(sink: Arc<Mutex<Vec<Option<String>>>>) -> WarrenForumDigestUpdater {
        WarrenForumDigestUpdater {
            api_url: "https://api.invalid".to_owned(),
            pinned_server_pubkey: Some(pin()),
            etag: None,
            highest_generation: 0,
            last: None,
            http: reqwest::Client::new(),
            on_update: Box::new(move |counts| {
                sink.lock().expect("sink lock").push(counts);
            }),
        }
    }

    #[test]
    fn a_verified_document_is_published_verbatim_for_the_renderer_to_index() {
        let published = Arc::new(Mutex::new(Vec::new()));
        let mut u = updater(published.clone());
        let json = signed_json(&[0, 3, 15], 4, now_unix() + 3600);

        u.last = Some(
            verify_forum_digest_any(&json, &[pin().as_str()]).expect("must verify under the pin"),
        );
        u.publish();

        assert_eq!(
            published.lock().expect("sink").as_slice(),
            [Some("03f".to_owned())],
            "the daemon verifies, the renderer indexes its own slot"
        );
    }

    #[test]
    fn an_expired_document_publishes_nothing_so_the_badge_drops() {
        let published = Arc::new(Mutex::new(Vec::new()));
        let mut u = updater(published.clone());
        let json = signed_json(&[9], 1, now_unix());

        u.last = Some(verify_forum_digest_any(&json, &[pin().as_str()]).expect("must verify"));
        u.publish();

        assert_eq!(
            published.lock().expect("sink").as_slice(),
            [None],
            "a server that stops answering must let the badge lapse, never freeze it"
        );
    }

    #[test]
    fn nothing_is_published_before_a_document_has_ever_verified() {
        let published = Arc::new(Mutex::new(Vec::new()));
        let u = updater(published.clone());

        u.publish();

        assert_eq!(published.lock().expect("sink").as_slice(), [None]);
    }

    #[test]
    fn a_document_signed_by_another_key_never_becomes_a_badge() {
        let other = SigningKey::from_bytes(&[0x55; 32]);
        let forged = sign_forum_digest(
            pack_unread_counts(&[9]),
            &other,
            1,
            now_unix(),
            now_unix() + 3600,
        );
        let json = serde_json::to_string(&forged).expect("serialize");

        let verified = verify_forum_digest_any(&json, &[pin().as_str()]);

        assert!(
            verified.is_err(),
            "anything able to answer for the API host could otherwise raise a badge that lures a click"
        );
    }

    #[test]
    fn a_verification_failure_reason_never_carries_envelope_values() {
        // The reason reaches the log, so it must stay a fixed phrase.
        for e in [
            ForumDigestError::BadSignature,
            ForumDigestError::InvalidCounts,
            ForumDigestError::InputTooLarge,
            ForumDigestError::UnsupportedVersion { got: 9 },
        ] {
            let reason = describe(&e);
            assert!(
                !reason.is_empty() && !reason.contains(char::is_numeric),
                "a reason must not carry values from the envelope: {reason}"
            );
        }
    }
}
