//! The server-signed launch announcements (`GET /v1/announcements`) on
//! Android: the daemon's `warren_announcements_updater` with the loop left to
//! Kotlin.
//!
//! An announcement is operator-authored text rendered verbatim on the home
//! screen next to a clickable link, which is a ready-made phishing surface. So
//! nothing reaches a screen unless the envelope verified against the pinned
//! server key just now, and the two properties that make a WITHDRAWAL real are
//! enforced here rather than by the server being polite:
//!
//! - the `generation` high-water mark, so a captured older envelope cannot put
//!   back a card the operator withdrew;
//! - the signed `expires_at`, re-applied on every read rather than only when
//!   the document changes, so a blocked or hostile network can suppress a card
//!   (it could suppress the whole API anyway) but can never freeze one on
//!   screen.
//!
//! No on-disk cache, exactly as on the desktop. The per-announcement TTL, the
//! client-version range and the call-to-action URL check are applied here too,
//! so Kotlin displays what it receives, verbatim, as plain text.
//!
//! The per-account voucher code cannot ride the broadcast document: that
//! document is byte-identical for every caller, which is what keeps the server
//! from learning who asks about what. It comes from a second, wallet-signed
//! call, and [`SessionCodes`] is what stops that call from being repeated on
//! every poll and from binding a code drawn for one wallet to another.

use std::collections::BTreeMap;

use serde::Serialize;
use warren_discovery_core::{
    AnnouncementsError, VerifiedAnnouncements, verify_signed_announcements_any,
};

/// What one GET brought back, as the transport saw it.
///
/// There is no `NotModified`: the shared API transport reads no response
/// header, so Android cannot carry an ETag across a poll the way the daemon
/// does. The anti-freeze property comes from the signed expiry rather than
/// from the validator, so the conditional GET would buy no safety here.
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

/// A call to action Kotlin may turn into a clickable button. Present only when
/// the contract's own URL check passed: withholding the button and never the
/// announcement is the deliberate split, so the operator's text still reaches
/// the reader while an unsafe link never becomes clickable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct DisplayCta {
    /// Button caption, plain text.
    pub(crate) label: String,
    /// Destination opened in the system browser, `https` only.
    pub(crate) url: String,
}

/// One announcement as handed to Kotlin: already filtered for expiry and
/// client version, with an unsafe call to action already withheld, so the card
/// displays it verbatim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct DisplayAnnouncement {
    /// Server-assigned id, forwarded so the UI can persist a dismissal.
    pub(crate) id: String,
    /// One-line title, rendered as plain text (never as markup).
    pub(crate) headline: String,
    /// Body text, rendered as plain text (never as markup).
    pub(crate) body: String,
    /// Severity, driving the card's indicator colour.
    pub(crate) level: &'static str,
    /// Call to action, `null` when there is none or its URL was refused.
    pub(crate) cta: Option<DisplayCta>,
    /// Campaign to ask for this account's code under, `null` when the
    /// announcement carries no offer. Its presence IS the offer, so Kotlin
    /// never has to reconcile a flag with an id.
    pub(crate) voucher_campaign_id: Option<String>,
}

/// The verified envelope held in memory, plus the fact that guards it.
pub(crate) struct AnnouncementsState {
    /// Highest generation ever accepted (anti-rollback high-water mark).
    highest_generation: u64,
    last: Option<VerifiedAnnouncements>,
}

impl AnnouncementsState {
    pub(crate) const fn new() -> Self {
        Self {
            highest_generation: 0,
            last: None,
        }
    }

    /// Applies one fetch: a body is verified against `pins` and refused when it
    /// would move the generation backwards (a replayed older envelope could put
    /// back a card the operator withdrew); a failure leaves the held envelope
    /// as it is, freshness deciding whether it still shows.
    pub(crate) fn accept(&mut self, fetched: Fetched, pins: &[&str]) -> Refresh {
        match fetched {
            Fetched::Transport => Refresh::Transport,
            Fetched::Status(status) => {
                log::debug!("announcements: endpoint returned {status}");
                Refresh::Rejected
            }
            Fetched::Body(body) => match verify_signed_announcements_any(&body, pins) {
                Ok(verified) => {
                    if verified.generation < self.highest_generation {
                        log::warn!(
                            "announcements: generation went backwards ({} < {}); ignoring",
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
                    log::error!("announcements: verification failed: {}", describe(&e));
                    Refresh::Rejected
                }
            },
        }
    }

    /// The announcements to display at `now`, empty once the envelope has
    /// lapsed. Re-applied on every read, which is what takes a card down on a
    /// client whose refresh is being blocked, with no timer in Kotlin.
    pub(crate) fn display(
        &self,
        now_unix: u64,
        client_version: Option<&str>,
    ) -> Vec<DisplayAnnouncement> {
        self.last
            .as_ref()
            .map(|verified| {
                verified
                    .active_for(now_unix, client_version)
                    .into_iter()
                    .map(|announcement| DisplayAnnouncement {
                        id: announcement.id.clone(),
                        headline: announcement.headline.clone(),
                        body: announcement.body.clone(),
                        level: crate::notices::level_token(announcement.level),
                        // The contract's own check, not a second copy of its
                        // rules: what is safe to render as a link is one
                        // decision, and it lives beside the wire format.
                        cta: announcement.displayable_cta().map(|safe| DisplayCta {
                            label: safe.label.clone(),
                            url: safe.url.clone(),
                        }),
                        voucher_campaign_id: announcement.voucher_campaign_id.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// The campaign codes this process holds, and the identity they were drawn
/// for.
///
/// In memory only, and dropped whole on an identity change: a code belongs to
/// the wallet that asked for it, and the wallet that replaces it has its own or
/// none at all. Re-fetching is always safe because the server call is a pure
/// lookup that never mints and never assigns.
///
/// Deliberately not zeroized. The code is rendered on the user's own screen and
/// crosses the FFI to get there, so wiping this one copy would be theatre; what
/// bounds the exposure is that it never reaches a log, an error or a problem
/// report.
pub(crate) struct SessionCodes {
    address: Option<String>,
    /// Campaign id to the code drawn for it. `None` records "outside the
    /// cohort", so a 404 is not re-asked on every poll.
    by_campaign: BTreeMap<String, Option<String>>,
}

/// What [`SessionCodes::held`] knows about a campaign before any request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Held {
    /// The answer for this identity is already known.
    Known(Option<String>),
    /// Nothing is held: the caller makes the signed request.
    Unknown,
}

impl SessionCodes {
    pub(crate) const fn new() -> Self {
        Self {
            address: None,
            by_campaign: BTreeMap::new(),
        }
    }

    /// What is held for `address` and `campaign_id`, dropping every code drawn
    /// for another identity first. Reading the identity BEFORE anything else is
    /// the same ordering `account_backend` uses before pulling a paid voucher:
    /// acting under a desynced identity would show one account the offer drawn
    /// for another.
    pub(crate) fn held(&mut self, address: &str, campaign_id: &str) -> Held {
        if self.address.as_deref() != Some(address) {
            self.address = Some(address.to_owned());
            self.by_campaign.clear();
            return Held::Unknown;
        }
        match self.by_campaign.get(campaign_id) {
            Some(code) => Held::Known(code.clone()),
            None => Held::Unknown,
        }
    }

    /// Records what the signed lookup answered for `address`. A code drawn for
    /// an identity that has since been replaced (create, restore, logout while
    /// the request was in flight) is dropped rather than kept.
    pub(crate) fn record(&mut self, address: &str, campaign_id: &str, code: Option<String>) {
        if self.address.as_deref() == Some(address) {
            self.by_campaign.insert(campaign_id.to_owned(), code);
        }
    }
}

/// The FFI envelope:
/// `{"announcements":[{"id":..,"headline":..,"body":..,"level":..,"cta":..,
/// "voucher_campaign_id":..}],"fetch":".."}`. The list is empty whenever
/// nothing is displayable, so Kotlin clears its card from the same signal that
/// raised it, and sizes its next delay on `fetch`.
pub(crate) fn envelope(announcements: &[DisplayAnnouncement], refresh: Refresh) -> String {
    #[derive(Serialize)]
    struct Envelope<'a> {
        announcements: &'a [DisplayAnnouncement],
        fetch: &'static str,
    }
    serde_json::to_string(&Envelope {
        announcements,
        fetch: refresh.token(),
    })
    .unwrap_or_else(|_| r#"{"announcements":[],"fetch":"rejected"}"#.to_owned())
}

/// The FFI envelope of the wallet-signed campaign lookup:
/// `{"ok":true,"code":"..."}` with the code this account was pre-assigned,
/// `{"ok":true,"code":null}` when the account is outside the cohort, and
/// `{"ok":false,"code":null}` when the lookup failed, which Kotlin retries
/// rather than reading as "you were never eligible".
pub(crate) fn voucher_envelope(outcome: Result<Option<String>, ()>) -> String {
    #[derive(Serialize)]
    struct Envelope<'a> {
        ok: bool,
        code: Option<&'a str>,
    }
    let (ok, code) = match &outcome {
        Ok(code) => (true, code.as_deref()),
        Err(()) => (false, None),
    };
    serde_json::to_string(&Envelope { ok, code })
        .unwrap_or_else(|_| r#"{"ok":false,"code":null}"#.to_owned())
}

/// Redacted one-line reason for a verification failure. Never carries the
/// pubkey or the body.
fn describe(e: &AnnouncementsError) -> &'static str {
    match e {
        AnnouncementsError::Json(_) => "malformed envelope",
        AnnouncementsError::UnsupportedVersion { .. } => "unsupported envelope version",
        AnnouncementsError::ServerPubkeyMismatch { .. } => "server pubkey is not the pinned one",
        AnnouncementsError::InvalidHex | AnnouncementsError::PubkeyNotOnCurve => {
            "malformed key or signature"
        }
        AnnouncementsError::BadSignature => "bad signature",
        AnnouncementsError::InputTooLarge => "envelope too large",
        _ => "verification failed",
    }
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::SigningKey;
    use warren_discovery_core::{
        Announcement, AnnouncementCta, NoticeLevel as WireLevel, sign_announcements,
    };

    use super::*;

    const NOW: u64 = 1_800_000_000;

    fn server_key() -> SigningKey {
        SigningKey::from_bytes(&[0x44; 32])
    }

    fn pin() -> String {
        hex::encode(server_key().verifying_key().as_bytes())
    }

    fn announcement(id: &str, headline: &str) -> Announcement {
        Announcement {
            id: id.to_owned(),
            headline: headline.to_owned(),
            body: "body".to_owned(),
            level: WireLevel::Info,
            cta: None,
            voucher_campaign_id: None,
            min_client_version: None,
            max_client_version: None,
            expires_at: None,
        }
    }

    fn signed_by(
        key: &SigningKey,
        announcements: Vec<Announcement>,
        generation: u64,
        expires_at: u64,
    ) -> Fetched {
        let signed = sign_announcements(announcements, key, generation, NOW, expires_at);
        Fetched::Body(serde_json::to_string(&signed).expect("serialize"))
    }

    fn signed(announcements: Vec<Announcement>, generation: u64, expires_at: u64) -> Fetched {
        signed_by(&server_key(), announcements, generation, expires_at)
    }

    #[test]
    fn a_verified_announcement_is_handed_over_verbatim_with_its_offer() {
        let mut state = AnnouncementsState::new();
        let pin = pin();
        let mut launch = announcement("a1", "Production is open");
        launch.body = "Your beta account gets a free month.".to_owned();
        launch.level = WireLevel::Warning;
        launch.cta = Some(AnnouncementCta {
            label: "Get Warren".to_owned(),
            url: "https://warren.ro/download".to_owned(),
        });
        launch.voucher_campaign_id = Some("prod-launch".to_owned());

        let refresh = state.accept(signed(vec![launch], 4, NOW + 3600), &[&pin]);

        assert_eq!(refresh, Refresh::Ok);
        assert_eq!(
            state.display(NOW, None),
            vec![DisplayAnnouncement {
                id: "a1".to_owned(),
                headline: "Production is open".to_owned(),
                body: "Your beta account gets a free month.".to_owned(),
                level: "warning",
                cta: Some(DisplayCta {
                    label: "Get Warren".to_owned(),
                    url: "https://warren.ro/download".to_owned(),
                }),
                voucher_campaign_id: Some("prod-launch".to_owned()),
            }],
            "the card must receive the operator's words untouched"
        );
    }

    #[test]
    fn nothing_is_held_before_an_envelope_has_ever_verified() {
        assert_eq!(AnnouncementsState::new().display(NOW, None), Vec::new());
    }

    #[test]
    fn an_envelope_signed_by_another_key_never_reaches_the_card() {
        let mut state = AnnouncementsState::new();
        let pin = pin();
        let other = SigningKey::from_bytes(&[0x55; 32]);

        let refresh = state.accept(
            signed_by(
                &other,
                vec![announcement("a1", "Claim your free month here")],
                1,
                NOW + 3600,
            ),
            &[&pin],
        );

        assert_eq!(refresh, Refresh::Rejected);
        assert_eq!(
            state.display(NOW, None),
            Vec::new(),
            "anything able to answer for the API host could otherwise put a link on every home screen"
        );
    }

    #[test]
    fn an_unparseable_body_is_ignored_rather_than_shown() {
        let mut state = AnnouncementsState::new();
        let pin = pin();

        let refresh = state.accept(Fetched::Body("not json at all".to_owned()), &[&pin]);

        assert_eq!(refresh, Refresh::Rejected);
        assert_eq!(state.display(NOW, None), Vec::new());
    }

    #[test]
    fn a_lapsed_envelope_shows_nothing_without_a_further_fetch() {
        // Anti-freeze: this is what clears the card on a client whose refresh
        // is being blocked, with no timer in Kotlin.
        let mut state = AnnouncementsState::new();
        let pin = pin();
        state.accept(
            signed(vec![announcement("a1", "stale")], 1, NOW + 60),
            &[&pin],
        );
        assert_eq!(state.display(NOW, None).len(), 1);

        assert_eq!(state.display(NOW + 60, None), Vec::new());
    }

    #[test]
    fn an_older_generation_is_ignored_and_the_newer_envelope_kept() {
        let mut state = AnnouncementsState::new();
        let pin = pin();
        state.accept(
            signed(vec![announcement("new", "current")], 7, NOW + 3600),
            &[&pin],
        );

        let refresh = state.accept(
            signed(vec![announcement("old", "withdrawn")], 6, NOW + 3600),
            &[&pin],
        );

        assert_eq!(refresh, Refresh::Rejected);
        assert_eq!(
            state
                .display(NOW, None)
                .into_iter()
                .map(|a| a.id)
                .collect::<Vec<_>>(),
            vec!["new".to_owned()],
            "a replayed older envelope could put back a card the operator withdrew"
        );
    }

    #[test]
    fn an_equal_generation_is_accepted_so_a_re_signed_set_keeps_the_card_alive() {
        let mut state = AnnouncementsState::new();
        let pin = pin();
        state.accept(
            signed(vec![announcement("a1", "live")], 5, NOW + 60),
            &[&pin],
        );

        let refresh = state.accept(
            signed(vec![announcement("a1", "live")], 5, NOW + 7200),
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
        let mut state = AnnouncementsState::new();
        let pin = pin();
        state.accept(
            signed(vec![announcement("a1", "live")], 1, NOW + 3600),
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
    fn a_version_targeted_announcement_is_withheld_from_an_out_of_range_client() {
        let mut state = AnnouncementsState::new();
        let pin = pin();
        let mut targeted = announcement("a1", "for 1.11 and up");
        targeted.min_client_version = Some("1.11.0".to_owned());
        state.accept(signed(vec![targeted], 1, NOW + 3600), &[&pin]);

        assert_eq!(
            state.display(NOW, Some("1.9.0")),
            Vec::new(),
            "the range is applied here, so the card never has to"
        );
        assert_eq!(state.display(NOW, Some("1.11.0")).len(), 1);
    }

    #[test]
    fn an_announcement_past_its_own_ttl_drops_while_the_envelope_still_stands() {
        let mut state = AnnouncementsState::new();
        let pin = pin();
        let mut short = announcement("a1", "for an hour");
        short.expires_at = Some(NOW + 60);
        state.accept(signed(vec![short], 1, NOW + 7200), &[&pin]);

        assert_eq!(state.display(NOW, None).len(), 1);
        assert_eq!(state.display(NOW + 60, None), Vec::new());
    }

    #[test]
    fn an_unsafe_call_to_action_is_withheld_while_the_text_still_reaches_the_reader() {
        let mut state = AnnouncementsState::new();
        let pin = pin();
        let mut phishy = announcement("a1", "Production is open");
        phishy.cta = Some(AnnouncementCta {
            label: "Claim it".to_owned(),
            url: "https://warren.ro@evil.example/claim".to_owned(),
        });
        state.accept(signed(vec![phishy], 1, NOW + 3600), &[&pin]);

        let shown = state.display(NOW, None);
        assert_eq!(shown.len(), 1, "the operator's text is not the unsafe part");
        assert_eq!(
            shown[0].cta, None,
            "a signature proves who wrote a URL, never that it is safe to click"
        );
    }

    #[test]
    fn the_envelope_carries_the_cards_and_the_fetch_class() {
        let announcements = vec![DisplayAnnouncement {
            id: "a1".to_owned(),
            headline: "quotes \" and \\ survive".to_owned(),
            body: "body".to_owned(),
            level: "info",
            cta: None,
            voucher_campaign_id: None,
        }];

        assert_eq!(
            envelope(&announcements, Refresh::Ok),
            r#"{"announcements":[{"id":"a1","headline":"quotes \" and \\ survive","body":"body","level":"info","cta":null,"voucher_campaign_id":null}],"fetch":"ok"}"#,
            "operator text is escaped by the serializer, never spliced into JSON by hand"
        );
        assert_eq!(
            envelope(&[], Refresh::Transport),
            r#"{"announcements":[],"fetch":"transport"}"#
        );
    }

    #[test]
    fn the_voucher_envelope_tells_an_empty_cohort_apart_from_a_failed_lookup() {
        // A transient outage must never tell a cohort member they were never
        // eligible, so the two answers are distinct tokens rather than one null.
        assert_eq!(
            voucher_envelope(Ok(Some("ABCD1234EFGH5678".to_owned()))),
            r#"{"ok":true,"code":"ABCD1234EFGH5678"}"#
        );
        assert_eq!(voucher_envelope(Ok(None)), r#"{"ok":true,"code":null}"#);
        assert_eq!(voucher_envelope(Err(())), r#"{"ok":false,"code":null}"#);
    }

    #[test]
    fn a_code_is_asked_for_once_per_identity_and_a_missing_one_is_not_re_asked() {
        let mut codes = SessionCodes::new();

        assert_eq!(codes.held("wbAAAA", "prod-launch"), Held::Unknown);
        codes.record("wbAAAA", "prod-launch", Some("CODE".to_owned()));
        assert_eq!(
            codes.held("wbAAAA", "prod-launch"),
            Held::Known(Some("CODE".to_owned()))
        );

        codes.record("wbAAAA", "other", None);
        assert_eq!(
            codes.held("wbAAAA", "other"),
            Held::Known(None),
            "outside the cohort is an answer, not a reason to ask again every poll"
        );
    }

    #[test]
    fn a_code_drawn_for_another_wallet_is_never_shown_to_this_one() {
        let mut codes = SessionCodes::new();
        codes.record("wbAAAA", "prod-launch", Some("CODE".to_owned()));
        codes.held("wbAAAA", "prod-launch");

        assert_eq!(
            codes.held("wbBBBB", "prod-launch"),
            Held::Unknown,
            "a code belongs to the wallet that asked for it"
        );
        // The record for the identity that has since been replaced is dropped,
        // not kept for a later switch back.
        codes.record("wbAAAA", "prod-launch", Some("CODE".to_owned()));
        assert_eq!(codes.held("wbBBBB", "prod-launch"), Held::Unknown);
    }

    #[test]
    fn a_verification_failure_reason_never_carries_envelope_values() {
        for e in [
            AnnouncementsError::BadSignature,
            AnnouncementsError::InvalidHex,
            AnnouncementsError::InputTooLarge,
            AnnouncementsError::UnsupportedVersion { got: 9 },
        ] {
            let reason = describe(&e);
            assert!(
                !reason.is_empty() && !reason.contains(char::is_numeric),
                "a reason must not carry values from the envelope: {reason}"
            );
        }
    }
}
