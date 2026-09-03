//! The broadcast forum activity digest (`GET /v1/forum/digest`) on Android:
//! the daemon's `warren_forum_digest_updater` with the loop left to Kotlin.
//!
//! The document is one anonymous array of unread counts, identical for every
//! client, so fetching it says nothing about the user: it carries no account,
//! and the cadence cannot be tied to one. The slot that turns the array into
//! a badge is known only to Kotlin, which holds it beside the forum handle,
//! so the counts are handed over verbatim and indexed there.
//!
//! What this module owns is everything that decides whether a fetched
//! document may become a badge: the signature against the pinned server key,
//! the anti-rollback high-water mark on `generation`, and the freshness that
//! is re-applied on every read so an envelope whose expiry passes while
//! nothing changes drops the badge by itself. No on-disk cache, and nothing
//! surfaced unless it verified.

use warren_discovery_core::{ForumDigestError, VerifiedForumDigest, verify_forum_digest_any};

/// What one conditional GET brought back, as the transport saw it.
pub(crate) enum Fetched {
    /// `304`: the held document is current.
    NotModified,
    /// `200`: a fresh body; `etag` is the response validator when the server
    /// sent one.
    Body { body: String, etag: Option<String> },
    /// Any other status.
    Status(u16),
    /// The request never got an HTTP answer.
    Transport,
}

/// Outcome of one refresh; what Kotlin's cadence reads. The tokens are the
/// FFI contract of [`envelope`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Refresh {
    /// A fresh document verified and is held.
    Ok,
    /// The server confirmed the held document is current.
    NotModified,
    /// Reached the server but did not accept the answer.
    Rejected,
    /// Never reached the server: the caller retries early.
    Transport,
}

impl Refresh {
    fn token(self) -> &'static str {
        match self {
            Refresh::Ok => "ok",
            Refresh::NotModified => "not-modified",
            Refresh::Rejected => "rejected",
            Refresh::Transport => "transport",
        }
    }
}

/// The verified document held in memory, plus the two facts that guard it.
pub(crate) struct DigestState {
    etag: Option<String>,
    /// Highest generation ever accepted (anti-rollback high-water mark).
    highest_generation: u64,
    last: Option<VerifiedForumDigest>,
}

impl DigestState {
    pub(crate) const fn new() -> Self {
        Self {
            etag: None,
            highest_generation: 0,
            last: None,
        }
    }

    /// The validator of the held document, for the next conditional GET.
    pub(crate) fn etag(&self) -> Option<String> {
        self.etag.clone()
    }

    /// Applies one fetch: a body is verified against `pins` and refused when
    /// it would move the generation backwards (a replayed older document could
    /// put back a badge the reader has already cleared); a 304 or a failure
    /// leaves the held document as it is, freshness deciding whether it still
    /// shows.
    pub(crate) fn accept(&mut self, fetched: Fetched, pins: &[&str]) -> Refresh {
        match fetched {
            Fetched::NotModified => Refresh::NotModified,
            Fetched::Transport => Refresh::Transport,
            Fetched::Status(status) => {
                log::debug!("forumDigest: endpoint returned {status}");
                Refresh::Rejected
            }
            Fetched::Body { body, etag } => match verify_forum_digest_any(&body, pins) {
                Ok(verified) => {
                    if verified.generation < self.highest_generation {
                        log::warn!(
                            "forumDigest: generation went backwards ({} < {}); ignoring",
                            verified.generation,
                            self.highest_generation
                        );
                        return Refresh::Rejected;
                    }
                    self.highest_generation = verified.generation;
                    // A server that stops sending a validator must not erase
                    // the last known one.
                    if etag.is_some() {
                        self.etag = etag;
                    }
                    self.last = Some(verified);
                    Refresh::Ok
                }
                Err(e) => {
                    // Never fall back to the body: a badge is an invitation to
                    // click, which is exactly what the signature exists to stop
                    // anyone else raising.
                    log::error!("forumDigest: verification failed: {}", describe(&e));
                    Refresh::Rejected
                }
            },
        }
    }

    /// The counts while the held document is fresh at `now`, `None`
    /// otherwise: a server that stops answering must let the badge lapse.
    pub(crate) fn counts(&self, now_unix: u64) -> Option<String> {
        self.last
            .as_ref()
            .filter(|verified| !verified.is_expired(now_unix))
            .map(|verified| verified.counts_hex().to_owned())
    }
}

/// The FFI envelope: `{"counts":"03f","fetch":"ok"}`, `counts` null while no
/// fresh document is held. Kotlin indexes its slot into `counts` and sizes its
/// next delay on `fetch`.
pub(crate) fn envelope(counts: Option<&str>, refresh: Refresh) -> String {
    let counts = counts.map_or("null".to_owned(), |c| format!("\"{c}\""));
    format!(r#"{{"counts":{counts},"fetch":"{}"}}"#, refresh.token())
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
    use ed25519_dalek::SigningKey;
    use warren_discovery_core::{pack_unread_counts, sign_forum_digest};

    use super::*;

    const NOW: u64 = 1_800_000_000;

    fn server_key() -> SigningKey {
        SigningKey::from_bytes(&[0x44; 32])
    }

    fn pin() -> String {
        hex::encode(server_key().verifying_key().as_bytes())
    }

    fn signed_json(counts: &[u32], generation: u64, expires_at: u64) -> String {
        signed_by(&server_key(), counts, generation, expires_at)
    }

    fn signed_by(key: &SigningKey, counts: &[u32], generation: u64, expires_at: u64) -> String {
        let signed =
            sign_forum_digest(pack_unread_counts(counts), key, generation, NOW, expires_at);
        serde_json::to_string(&signed).expect("serialize")
    }

    fn body(json: String) -> Fetched {
        Fetched::Body {
            body: json,
            etag: Some("\"v1\"".to_owned()),
        }
    }

    #[test]
    fn a_verified_document_is_handed_over_verbatim_for_kotlin_to_index() {
        let mut state = DigestState::new();
        let pin = pin();

        let refresh = state.accept(body(signed_json(&[0, 3, 15], 4, NOW + 3600)), &[&pin]);

        assert_eq!(refresh, Refresh::Ok);
        assert_eq!(state.counts(NOW).as_deref(), Some("03f"));
        assert_eq!(state.etag().as_deref(), Some("\"v1\""));
    }

    #[test]
    fn an_expired_document_yields_nothing_so_the_badge_drops() {
        let mut state = DigestState::new();
        let pin = pin();
        state.accept(body(signed_json(&[9], 1, NOW + 60)), &[&pin]);
        assert_eq!(state.counts(NOW).as_deref(), Some("9"));

        // Nothing changed on the server and nothing was fetched: the expiry
        // alone must take the badge down.
        assert_eq!(state.counts(NOW + 60), None);
    }

    #[test]
    fn nothing_is_held_before_a_document_has_ever_verified() {
        let state = DigestState::new();
        assert_eq!(state.counts(NOW), None);
        assert_eq!(state.etag(), None);
    }

    #[test]
    fn a_document_signed_by_another_key_never_becomes_a_badge() {
        let mut state = DigestState::new();
        let pin = pin();
        let other = SigningKey::from_bytes(&[0x55; 32]);

        let refresh = state.accept(body(signed_by(&other, &[9], 1, NOW + 3600)), &[&pin]);

        assert_eq!(refresh, Refresh::Rejected);
        assert_eq!(
            state.counts(NOW),
            None,
            "anything able to answer for the API host could otherwise raise a badge that lures a click"
        );
    }

    #[test]
    fn an_older_generation_is_ignored_and_the_newer_document_kept() {
        let mut state = DigestState::new();
        let pin = pin();
        state.accept(body(signed_json(&[0], 7, NOW + 3600)), &[&pin]);

        let refresh = state.accept(body(signed_json(&[9], 6, NOW + 3600)), &[&pin]);

        assert_eq!(refresh, Refresh::Rejected);
        assert_eq!(
            state.counts(NOW).as_deref(),
            Some("0"),
            "a replayed older document could put back a badge the reader has already cleared"
        );
    }

    #[test]
    fn a_not_modified_and_a_failed_fetch_keep_the_held_document() {
        let mut state = DigestState::new();
        let pin = pin();
        state.accept(body(signed_json(&[2], 1, NOW + 3600)), &[&pin]);

        assert_eq!(
            state.accept(Fetched::NotModified, &[&pin]),
            Refresh::NotModified
        );
        assert_eq!(
            state.accept(Fetched::Transport, &[&pin]),
            Refresh::Transport
        );
        assert_eq!(
            state.accept(Fetched::Status(503), &[&pin]),
            Refresh::Rejected
        );
        assert_eq!(state.counts(NOW).as_deref(), Some("2"));
    }

    #[test]
    fn a_body_without_a_validator_keeps_the_last_known_one() {
        let mut state = DigestState::new();
        let pin = pin();
        state.accept(body(signed_json(&[1], 1, NOW + 3600)), &[&pin]);

        state.accept(
            Fetched::Body {
                body: signed_json(&[2], 2, NOW + 3600),
                etag: None,
            },
            &[&pin],
        );

        assert_eq!(state.etag().as_deref(), Some("\"v1\""));
        assert_eq!(state.counts(NOW).as_deref(), Some("2"));
    }

    #[test]
    fn the_envelope_carries_the_counts_or_null_and_the_fetch_class() {
        assert_eq!(
            envelope(Some("03f"), Refresh::Ok),
            r#"{"counts":"03f","fetch":"ok"}"#
        );
        assert_eq!(
            envelope(None, Refresh::Transport),
            r#"{"counts":null,"fetch":"transport"}"#
        );
        assert_eq!(
            envelope(Some("0"), Refresh::NotModified),
            r#"{"counts":"0","fetch":"not-modified"}"#
        );
        assert_eq!(
            envelope(None, Refresh::Rejected),
            r#"{"counts":null,"fetch":"rejected"}"#
        );
    }

    #[test]
    fn a_verification_failure_reason_never_carries_envelope_values() {
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
