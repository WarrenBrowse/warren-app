//! The operator broadcast notices (`GET /v1/notices`) on Android: the daemon's
//! `warren_notices_updater` with the loop left to Kotlin.
//!
//! A notice is operator-authored text rendered verbatim on the home screen,
//! which makes an unauthenticated channel a ready-made phishing surface. So
//! nothing reaches a screen unless the envelope verified against the pinned
//! server key just now, and the two properties that make an ERASURE real are
//! enforced here, not by the server being polite:
//!
//! - the `generation` high-water mark, so a captured older envelope cannot
//!   resurrect a notice the operator deleted;
//! - the signed `expires_at`, re-applied on every read rather than only when
//!   the document changes, so a blocked or hostile network can suppress a
//!   notice (it could suppress the whole API anyway) but can never freeze one
//!   on screen.
//!
//! No on-disk cache, exactly as on the desktop: a notice is a live statement,
//! and persisting one would only create a way for an erased message to come
//! back on a client that cannot reach the API.
//!
//! The per-notice TTL and the client-version range are applied here too, so
//! Kotlin displays what it receives, verbatim, as plain text.

use serde::Serialize;
use warren_discovery_core::{NoticesError, VerifiedNotices, verify_signed_notices_any};

/// What one GET brought back, as the transport saw it.
///
/// There is no `NotModified`: the shared API transport reads no response
/// header, so Android cannot carry an ETag across a poll the way the daemon
/// does. The document is a few hundred bytes per notice and the poll is one
/// every five minutes while the app is on screen, so the conditional GET buys
/// little; it bought no safety either, since the anti-freeze property comes
/// from the signed expiry rather than from the validator.
pub(crate) enum Fetched {
    /// `200`: a fresh body.
    Body(String),
    /// Any other status.
    Status(u16),
    /// The request never got an HTTP answer.
    Transport,
}

/// Outcome of one refresh; what Kotlin's cadence reads. The tokens are the
/// FFI contract of [`envelope`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Refresh {
    /// A fresh envelope verified and is held.
    Ok,
    /// Reached the server but did not accept the answer.
    Rejected,
    /// Never reached the server: the caller retries early.
    Transport,
}

impl Refresh {
    fn token(self) -> &'static str {
        match self {
            Refresh::Ok => "ok",
            Refresh::Rejected => "rejected",
            Refresh::Transport => "transport",
        }
    }
}

/// One notice as handed to Kotlin: already filtered for expiry and client
/// version, so the banner displays it verbatim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct DisplayNotice {
    /// Server-assigned id, forwarded so the UI can keep per-notice state.
    pub(crate) id: String,
    /// Operator text, rendered as plain text (never as markup).
    pub(crate) message: String,
    /// Severity, driving the banner's title and indicator colour.
    pub(crate) level: &'static str,
}

/// The wire enum as the FFI spells it. Kept as fixed tokens rather than the
/// contract type's serde name so a rotation of the wire spelling cannot
/// silently change what Kotlin matches on.
fn level_token(level: warren_discovery_core::NoticeLevel) -> &'static str {
    use warren_discovery_core::NoticeLevel as Wire;
    match level {
        Wire::Info => "info",
        Wire::Warning => "warning",
        Wire::Error => "error",
    }
}

/// The verified envelope held in memory, plus the fact that guards it.
pub(crate) struct NoticesState {
    /// Highest generation ever accepted (anti-rollback high-water mark).
    highest_generation: u64,
    last: Option<VerifiedNotices>,
}

impl NoticesState {
    pub(crate) const fn new() -> Self {
        Self {
            highest_generation: 0,
            last: None,
        }
    }

    /// Applies one fetch: a body is verified against `pins` and refused when
    /// it would move the generation backwards (a replayed older envelope could
    /// put back a notice the operator erased); a failure leaves the held
    /// envelope as it is, freshness deciding whether it still shows.
    pub(crate) fn accept(&mut self, fetched: Fetched, pins: &[&str]) -> Refresh {
        match fetched {
            Fetched::Transport => Refresh::Transport,
            Fetched::Status(status) => {
                log::debug!("notices: endpoint returned {status}");
                Refresh::Rejected
            }
            Fetched::Body(body) => match verify_signed_notices_any(&body, pins) {
                Ok(verified) => {
                    if verified.generation < self.highest_generation {
                        log::warn!(
                            "notices: generation went backwards ({} < {}); ignoring",
                            verified.generation,
                            self.highest_generation
                        );
                        return Refresh::Rejected;
                    }
                    self.highest_generation = verified.generation;
                    self.last = Some(verified);
                    Refresh::Ok
                }
                Err(e) => {
                    // Never fall back to the body: an unverified envelope is
                    // exactly the phishing case the signature exists to stop.
                    log::error!("notices: verification failed: {}", describe(&e));
                    Refresh::Rejected
                }
            },
        }
    }

    /// The notices to display at `now`, empty once the envelope has lapsed.
    /// Re-applied on every read, which is what takes a banner down on a client
    /// whose refresh is being blocked, with no timer in Kotlin.
    pub(crate) fn display(
        &self,
        now_unix: u64,
        client_version: Option<&str>,
    ) -> Vec<DisplayNotice> {
        self.last
            .as_ref()
            .map(|verified| {
                verified
                    .active_for(now_unix, client_version)
                    .into_iter()
                    .map(|notice| DisplayNotice {
                        id: notice.id.clone(),
                        message: notice.message.clone(),
                        level: level_token(notice.level),
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// The FFI envelope: `{"notices":[{"id":..,"message":..,"level":..}],"fetch":".."}`.
/// The list is empty whenever nothing is displayable, so Kotlin clears its
/// banner from the same signal that raised it, and sizes its next delay on
/// `fetch`.
pub(crate) fn envelope(notices: &[DisplayNotice], refresh: Refresh) -> String {
    #[derive(Serialize)]
    struct Envelope<'a> {
        notices: &'a [DisplayNotice],
        fetch: &'static str,
    }
    serde_json::to_string(&Envelope {
        notices,
        fetch: refresh.token(),
    })
    .unwrap_or_else(|_| r#"{"notices":[],"fetch":"rejected"}"#.to_owned())
}

/// Redacted one-line reason for a verification failure. Never carries the
/// pubkey or the body.
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
    use ed25519_dalek::SigningKey;
    use warren_discovery_core::{Notice, NoticeLevel as WireLevel, sign_notices};

    use super::*;

    const NOW: u64 = 1_800_000_000;

    fn server_key() -> SigningKey {
        SigningKey::from_bytes(&[0x44; 32])
    }

    fn pin() -> String {
        hex::encode(server_key().verifying_key().as_bytes())
    }

    fn notice(id: &str, message: &str, level: WireLevel) -> Notice {
        Notice {
            id: id.to_owned(),
            message: message.to_owned(),
            level,
            min_client_version: None,
            max_client_version: None,
            expires_at: None,
        }
    }

    fn signed_by(
        key: &SigningKey,
        notices: Vec<Notice>,
        generation: u64,
        expires_at: u64,
    ) -> Fetched {
        let signed = sign_notices(notices, key, generation, NOW, expires_at);
        Fetched::Body(serde_json::to_string(&signed).expect("serialize"))
    }

    fn signed(notices: Vec<Notice>, generation: u64, expires_at: u64) -> Fetched {
        signed_by(&server_key(), notices, generation, expires_at)
    }

    #[test]
    fn a_verified_notice_is_handed_over_verbatim_with_its_severity() {
        let mut state = NoticesState::new();
        let pin = pin();

        let refresh = state.accept(
            signed(
                vec![notice("a1", "exit outage in NL", WireLevel::Error)],
                4,
                NOW + 3600,
            ),
            &[&pin],
        );

        assert_eq!(refresh, Refresh::Ok);
        assert_eq!(
            state.display(NOW, None),
            vec![DisplayNotice {
                id: "a1".to_owned(),
                message: "exit outage in NL".to_owned(),
                level: "error",
            }],
            "the banner must receive the operator's words untouched"
        );
    }

    #[test]
    fn nothing_is_held_before_an_envelope_has_ever_verified() {
        assert_eq!(NoticesState::new().display(NOW, None), Vec::new());
    }

    #[test]
    fn an_envelope_signed_by_another_key_never_reaches_the_banner() {
        let mut state = NoticesState::new();
        let pin = pin();
        let other = SigningKey::from_bytes(&[0x55; 32]);

        let refresh = state.accept(
            signed_by(
                &other,
                vec![notice(
                    "a1",
                    "send us your recovery phrase",
                    WireLevel::Error,
                )],
                1,
                NOW + 3600,
            ),
            &[&pin],
        );

        assert_eq!(refresh, Refresh::Rejected);
        assert_eq!(
            state.display(NOW, None),
            Vec::new(),
            "anything able to answer for the API host could otherwise put words in the product's mouth"
        );
    }

    #[test]
    fn an_unparseable_body_is_ignored_rather_than_shown() {
        let mut state = NoticesState::new();
        let pin = pin();

        let refresh = state.accept(Fetched::Body("not json at all".to_owned()), &[&pin]);

        assert_eq!(refresh, Refresh::Rejected);
        assert_eq!(state.display(NOW, None), Vec::new());
    }

    #[test]
    fn a_lapsed_envelope_shows_nothing_without_a_further_fetch() {
        // Anti-freeze: this is what clears the banner on a client whose
        // refresh is being blocked, with no timer in Kotlin.
        let mut state = NoticesState::new();
        let pin = pin();
        state.accept(
            signed(vec![notice("a1", "stale", WireLevel::Info)], 1, NOW + 60),
            &[&pin],
        );
        assert_eq!(state.display(NOW, None).len(), 1);

        assert_eq!(state.display(NOW + 60, None), Vec::new());
    }

    #[test]
    fn an_older_generation_is_ignored_and_the_newer_envelope_kept() {
        let mut state = NoticesState::new();
        let pin = pin();
        state.accept(
            signed(
                vec![notice("new", "current", WireLevel::Info)],
                7,
                NOW + 3600,
            ),
            &[&pin],
        );

        let refresh = state.accept(
            signed(
                vec![notice("old", "erased", WireLevel::Info)],
                6,
                NOW + 3600,
            ),
            &[&pin],
        );

        assert_eq!(refresh, Refresh::Rejected);
        assert_eq!(
            state
                .display(NOW, None)
                .into_iter()
                .map(|n| n.id)
                .collect::<Vec<_>>(),
            vec!["new".to_owned()],
            "a replayed older envelope could put back a notice the operator erased"
        );
    }

    #[test]
    fn an_equal_generation_is_accepted_so_a_re_signed_set_keeps_the_banner_alive() {
        let mut state = NoticesState::new();
        let pin = pin();
        state.accept(
            signed(vec![notice("a1", "live", WireLevel::Info)], 5, NOW + 60),
            &[&pin],
        );

        let refresh = state.accept(
            signed(vec![notice("a1", "live", WireLevel::Info)], 5, NOW + 7200),
            &[&pin],
        );

        assert_eq!(refresh, Refresh::Ok);
        assert_eq!(
            state.display(NOW + 3600, None).len(),
            1,
            "the server re-signs the same set periodically; refusing that would strand \
             the client on an envelope that is about to expire"
        );
    }

    #[test]
    fn a_failed_fetch_keeps_the_held_envelope() {
        let mut state = NoticesState::new();
        let pin = pin();
        state.accept(
            signed(vec![notice("a1", "live", WireLevel::Info)], 1, NOW + 3600),
            &[&pin],
        );

        assert_eq!(
            state.accept(Fetched::Transport, &[&pin]),
            Refresh::Transport
        );
        assert_eq!(
            state.accept(Fetched::Status(503), &[&pin]),
            Refresh::Rejected
        );
        assert_eq!(state.display(NOW, None).len(), 1);
    }

    #[test]
    fn a_version_targeted_notice_is_withheld_from_an_out_of_range_client() {
        let mut state = NoticesState::new();
        let pin = pin();
        let mut targeted = notice("a1", "for 1.11 and up", WireLevel::Warning);
        targeted.min_client_version = Some("1.11.0".to_owned());
        state.accept(signed(vec![targeted], 1, NOW + 3600), &[&pin]);

        assert_eq!(
            state.display(NOW, Some("1.9.0")),
            Vec::new(),
            "the range is applied here, so the banner never has to"
        );
        assert_eq!(state.display(NOW, Some("1.11.0")).len(), 1);
    }

    #[test]
    fn a_notice_past_its_own_ttl_drops_while_the_envelope_still_stands() {
        let mut state = NoticesState::new();
        let pin = pin();
        let mut short = notice("a1", "for an hour", WireLevel::Info);
        short.expires_at = Some(NOW + 60);
        state.accept(signed(vec![short], 1, NOW + 7200), &[&pin]);

        assert_eq!(state.display(NOW, None).len(), 1);
        assert_eq!(state.display(NOW + 60, None), Vec::new());
    }

    #[test]
    fn the_envelope_carries_the_notices_and_the_fetch_class() {
        let notices = vec![DisplayNotice {
            id: "a1".to_owned(),
            message: "quotes \" and \\ survive".to_owned(),
            level: "warning",
        }];

        assert_eq!(
            envelope(&notices, Refresh::Ok),
            r#"{"notices":[{"id":"a1","message":"quotes \" and \\ survive","level":"warning"}],"fetch":"ok"}"#,
            "operator text is escaped by the serializer, never spliced into JSON by hand"
        );
        assert_eq!(
            envelope(&[], Refresh::Transport),
            r#"{"notices":[],"fetch":"transport"}"#
        );
        assert_eq!(
            envelope(&[], Refresh::Rejected),
            r#"{"notices":[],"fetch":"rejected"}"#
        );
    }

    #[test]
    fn a_verification_failure_reason_never_carries_envelope_values() {
        for e in [
            NoticesError::BadSignature,
            NoticesError::InvalidHex,
            NoticesError::InputTooLarge,
            NoticesError::UnsupportedVersion { got: 9 },
        ] {
            let reason = describe(&e);
            assert!(
                !reason.is_empty() && !reason.contains(char::is_numeric),
                "a reason must not carry values from the envelope: {reason}"
            );
        }
    }
}
