//! The server-signed launch announcements (`GET /v1/announcements`) on iOS.
//!
//! Swift fetches the envelope with URLSession, exactly as it fetches the
//! update manifest, and hands the raw bytes here. Everything that decides
//! whether a card may be shown at all happens on this side: the Ed25519
//! signature against the pinned server key, the anti-rollback generation, the
//! signed envelope expiry, each announcement's own TTL, the client-version
//! range, and whether the call to action is safe to render as a link.
//!
//! An announcement is operator-authored text rendered verbatim on the home
//! screen next to a clickable link, which is a ready-made phishing surface.
//! Hence the pinned key, and hence the two properties that make a WITHDRAWAL
//! real rather than a matter of the server being polite:
//!
//! - the `generation` high-water mark, so a captured older envelope cannot put
//!   back a card the operator withdrew;
//! - `active_until`, the signed envelope expiry handed to Swift, so a blocked
//!   or hostile network can suppress a card (it could suppress the whole API
//!   anyway) but can never freeze one on screen.
//!
//! No on-disk cache, like the desktop daemon and Android: a withdrawn
//! announcement must not come back off a disk the API can no longer correct,
//! and the voucher code the offer carries must not outlive the campaign there.
//!
//! The per-account voucher code cannot ride this document: it is
//! byte-identical for every caller, which is what keeps the server from
//! learning who asks about what. It comes from the second, wallet-signed call
//! in `warren_account_ffi`.

use std::ffi::{CStr, CString, c_char};
use std::sync::Mutex;

use serde::Serialize;
use warren_discovery_core::{AnnouncementsError, verify_signed_announcements_any};

/// A call to action Swift may turn into a button. Present only when the
/// contract's own URL check passed: withholding the button and never the
/// announcement is the deliberate split, so the operator's text still reaches
/// the reader while an unsafe link never becomes tappable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct DisplayCta {
    /// Button caption, plain text.
    pub(crate) label: String,
    /// Destination opened through the app's external-link path, `https` only.
    pub(crate) url: String,
}

/// One announcement as handed to Swift: already filtered for expiry and client
/// version, with an unsafe call to action already withheld, so the card
/// displays it verbatim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct DisplayAnnouncement {
    /// Server-assigned id, forwarded so the app can persist a dismissal.
    pub(crate) id: String,
    /// One-line title, rendered as plain text (never as markup).
    pub(crate) headline: String,
    /// Body text, rendered as plain text (never as markup).
    pub(crate) body: String,
    /// Severity, driving the banner's indicator colour.
    pub(crate) level: &'static str,
    /// Call to action, `null` when there is none or its URL was refused.
    pub(crate) cta: Option<DisplayCta>,
    /// Campaign to ask for this account's code under, `null` when the
    /// announcement carries no offer. Its presence IS the offer, so Swift
    /// never has to reconcile a flag with an id.
    pub(crate) voucher_campaign_id: Option<String>,
}

/// What one accepted envelope hands to Swift.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct Verified {
    /// The announcements to display right now, in publication order.
    pub(crate) announcements: Vec<DisplayAnnouncement>,
    /// The signed envelope expiry, unix seconds. Swift stops displaying the
    /// held set at that instant without a further fetch, which is what takes a
    /// card down on a client whose refresh is being blocked. Each
    /// announcement's own TTL is applied here instead, and re-applied on the
    /// next fetch.
    pub(crate) active_until: u64,
}

/// The anti-rollback high-water mark this process has reached.
///
/// The only thing carried between calls. The verified envelope itself is not
/// held: Swift keeps what it was handed, so a re-read needs no second entry
/// point and a failed fetch leaves the card exactly as it was.
pub(crate) struct AnnouncementsVerifier {
    highest_generation: u64,
}

impl AnnouncementsVerifier {
    pub(crate) const fn new() -> Self {
        Self {
            highest_generation: 0,
        }
    }

    /// Verifies one fetched envelope and renders what is displayable at
    /// `now_unix`. `None` on any refusal, and a refusal is never a reason to
    /// take a card down: Swift keeps the set it holds until its `active_until`
    /// passes.
    pub(crate) fn accept(
        &mut self,
        body: &str,
        pins: &[&str],
        now_unix: u64,
        client_version: Option<&str>,
    ) -> Option<Verified> {
        let verified = match verify_signed_announcements_any(body, pins) {
            Ok(verified) => verified,
            Err(e) => {
                // Never fall back to the body: an unverified envelope is
                // exactly the phishing case the signature exists to stop.
                tracing::error!("announcements: verification failed: {}", describe(&e));
                return None;
            }
        };
        if verified.generation < self.highest_generation {
            tracing::warn!(
                "announcements: generation went backwards ({} < {}); ignoring",
                verified.generation,
                self.highest_generation
            );
            return None;
        }
        self.highest_generation = verified.generation;
        let announcements = verified
            .active_for(now_unix, client_version)
            .into_iter()
            .map(|announcement| DisplayAnnouncement {
                id: announcement.id.clone(),
                headline: announcement.headline.clone(),
                body: announcement.body.clone(),
                level: level_token(announcement.level),
                // The contract's own check, not a second copy of its rules:
                // what is safe to render as a link is one decision, and it
                // lives beside the wire format.
                cta: announcement.displayable_cta().map(|safe| DisplayCta {
                    label: safe.label.clone(),
                    url: safe.url.clone(),
                }),
                voucher_campaign_id: announcement.voucher_campaign_id.clone(),
            })
            .collect();
        Some(Verified {
            announcements,
            active_until: verified.expires_at,
        })
    }
}

/// The token Swift maps to a banner style.
fn level_token(level: warren_discovery_core::NoticeLevel) -> &'static str {
    use warren_discovery_core::NoticeLevel as Wire;
    match level {
        Wire::Info => "info",
        Wire::Warning => "warning",
        Wire::Error => "error",
    }
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

/// The high-water mark this process has reached. In memory only, exactly like
/// Android's: it must not survive a reinstall as a floor nothing can lower.
static VERIFIER: Mutex<AnnouncementsVerifier> = Mutex::new(AnnouncementsVerifier::new());

/// Verify a fetched `GET /v1/announcements` envelope and render what this
/// build must display right now.
///
/// `envelope` / `envelope_len`: raw bytes of the fetched body.
/// `current_version`: null-terminated running app version string, used for the
/// per-announcement client-version range. An empty string withholds every
/// range-targeted announcement, because a targeted card shown to an untargeted
/// client is worse than one not shown.
///
/// Returns a heap-allocated JSON C string
/// `{"announcements":[{"id":..,"headline":..,"body":..,"level":..,"cta":..,
/// "voucher_campaign_id":..}],"active_until":<unix secs>}`. Returns null when
/// the envelope does not verify, moves the generation backwards, or is
/// unreadable: the caller must then keep what it already holds and never trust
/// the body.
///
/// # Safety
/// `envelope` must point to `envelope_len` readable bytes. `current_version`
/// must be a valid null-terminated C string. The returned pointer must be
/// passed to `warren_announcements_free` exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn warren_announcements_verify(
    envelope: *const u8,
    envelope_len: usize,
    current_version: *const c_char,
) -> *mut c_char {
    if envelope.is_null() || current_version.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: `envelope` points to `envelope_len` readable bytes (fn precondition).
    let bytes = unsafe { std::slice::from_raw_parts(envelope, envelope_len) };
    let Ok(body) = std::str::from_utf8(bytes) else {
        return std::ptr::null_mut();
    };
    // SAFETY: `current_version` is a valid null-terminated C string (fn precondition).
    let Ok(version) = (unsafe { CStr::from_ptr(current_version) }).to_str() else {
        return std::ptr::null_mut();
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let client_version = Some(version).filter(|v| !v.is_empty());
    let pins = [crate::warren_product_config::WARREN_SERVER_PUBKEY_HEX];
    let Ok(mut verifier) = VERIFIER.lock() else {
        return std::ptr::null_mut();
    };
    let Some(verified) = verifier.accept(body, &pins, now, client_version) else {
        return std::ptr::null_mut();
    };
    drop(verifier);
    let Ok(json) = serde_json::to_string(&verified) else {
        return std::ptr::null_mut();
    };
    match CString::new(json) {
        Ok(s) => s.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Frees a string previously returned by `warren_announcements_verify`. No-op
/// on null.
///
/// # Safety
/// `ptr` must have been returned by `warren_announcements_verify` and must not
/// have been freed already.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn warren_announcements_free(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    // SAFETY: `ptr` came from `CString::into_raw` and is not yet freed (fn precondition).
    drop(unsafe { CString::from_raw(ptr) });
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
    ) -> String {
        let signed = sign_announcements(announcements, key, generation, NOW, expires_at);
        serde_json::to_string(&signed).expect("serialize")
    }

    fn signed(announcements: Vec<Announcement>, generation: u64, expires_at: u64) -> String {
        signed_by(&server_key(), announcements, generation, expires_at)
    }

    #[test]
    fn a_verified_announcement_is_handed_over_verbatim_with_its_offer() {
        let mut verifier = AnnouncementsVerifier::new();
        let pin = pin();
        let mut launch = announcement("a1", "Production is open");
        launch.body = "Your beta account gets a free month.".to_owned();
        launch.level = WireLevel::Warning;
        launch.cta = Some(AnnouncementCta {
            label: "Get Warren".to_owned(),
            url: "https://warren.ro/download".to_owned(),
        });
        launch.voucher_campaign_id = Some("prod-launch".to_owned());

        let verified = verifier
            .accept(&signed(vec![launch], 4, NOW + 3600), &[&pin], NOW, None)
            .expect("a pinned-key envelope must verify");

        assert_eq!(
            verified.announcements,
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
        assert_eq!(
            verified.active_until,
            NOW + 3600,
            "Swift stops displaying the held set at the signed expiry"
        );
    }

    #[test]
    fn an_envelope_signed_by_another_key_never_reaches_the_card() {
        let mut verifier = AnnouncementsVerifier::new();
        let pin = pin();
        let other = SigningKey::from_bytes(&[0x55; 32]);

        assert!(
            verifier
                .accept(
                    &signed_by(
                        &other,
                        vec![announcement("a1", "Claim your free month here")],
                        1,
                        NOW + 3600,
                    ),
                    &[&pin],
                    NOW,
                    None,
                )
                .is_none(),
            "anything able to answer for the API host could otherwise put a link on every home screen"
        );
    }

    #[test]
    fn an_unparseable_body_is_refused_rather_than_shown() {
        let mut verifier = AnnouncementsVerifier::new();
        let pin = pin();

        assert!(
            verifier
                .accept("not json at all", &[&pin], NOW, None)
                .is_none()
        );
        assert!(verifier.accept("", &[&pin], NOW, None).is_none());
    }

    #[test]
    fn a_lapsed_envelope_yields_nothing_to_display() {
        let mut verifier = AnnouncementsVerifier::new();
        let pin = pin();

        let verified = verifier
            .accept(
                &signed(vec![announcement("a1", "stale")], 1, NOW),
                &[&pin],
                NOW,
                None,
            )
            .expect("the signature still verifies, the content has simply lapsed");

        assert_eq!(verified.announcements, Vec::new());
    }

    #[test]
    fn an_older_generation_is_refused_so_a_withdrawn_card_cannot_come_back() {
        let mut verifier = AnnouncementsVerifier::new();
        let pin = pin();
        verifier
            .accept(
                &signed(vec![announcement("new", "current")], 7, NOW + 3600),
                &[&pin],
                NOW,
                None,
            )
            .expect("the newer envelope verifies");

        assert!(
            verifier
                .accept(
                    &signed(vec![announcement("old", "withdrawn")], 6, NOW + 3600),
                    &[&pin],
                    NOW,
                    None,
                )
                .is_none(),
            "a replayed older envelope could put back a card the operator withdrew"
        );
    }

    #[test]
    fn an_equal_generation_is_accepted_so_a_re_signed_set_keeps_the_card_alive() {
        let mut verifier = AnnouncementsVerifier::new();
        let pin = pin();
        verifier
            .accept(
                &signed(vec![announcement("a1", "live")], 5, NOW + 60),
                &[&pin],
                NOW,
                None,
            )
            .expect("the first envelope verifies");

        let verified = verifier
            .accept(
                &signed(vec![announcement("a1", "live")], 5, NOW + 7200),
                &[&pin],
                NOW,
                None,
            )
            .expect(
                "the server re-signs the same set periodically; refusing that would strand the \
                 client on an envelope that is about to expire",
            );

        assert_eq!(verified.active_until, NOW + 7200);
    }

    #[test]
    fn a_version_targeted_announcement_is_withheld_from_an_out_of_range_client() {
        let mut verifier = AnnouncementsVerifier::new();
        let pin = pin();
        let mut targeted = announcement("a1", "for 1.11 and up");
        targeted.min_client_version = Some("1.11.0".to_owned());
        let envelope = signed(vec![targeted], 1, NOW + 3600);

        let withheld = verifier
            .accept(&envelope, &[&pin], NOW, Some("1.9.0"))
            .expect("the envelope verifies");
        assert_eq!(
            withheld.announcements,
            Vec::new(),
            "the range is applied here, so the card never has to"
        );

        let shown = verifier
            .accept(&envelope, &[&pin], NOW, Some("1.11.0"))
            .expect("the envelope verifies");
        assert_eq!(shown.announcements.len(), 1);
    }

    #[test]
    fn an_announcement_past_its_own_ttl_drops_while_the_envelope_still_stands() {
        let mut verifier = AnnouncementsVerifier::new();
        let pin = pin();
        let mut short = announcement("a1", "for an hour");
        short.expires_at = Some(NOW + 60);
        let envelope = signed(vec![short], 1, NOW + 7200);

        assert_eq!(
            verifier
                .accept(&envelope, &[&pin], NOW, None)
                .expect("verifies")
                .announcements
                .len(),
            1
        );
        assert_eq!(
            verifier
                .accept(&envelope, &[&pin], NOW + 60, None)
                .expect("verifies")
                .announcements,
            Vec::new()
        );
    }

    #[test]
    fn an_unsafe_call_to_action_is_withheld_while_the_text_still_reaches_the_reader() {
        let mut verifier = AnnouncementsVerifier::new();
        let pin = pin();
        let mut phishy = announcement("a1", "Production is open");
        phishy.cta = Some(AnnouncementCta {
            label: "Claim it".to_owned(),
            url: "https://warren.ro@evil.example/claim".to_owned(),
        });

        let verified = verifier
            .accept(&signed(vec![phishy], 1, NOW + 3600), &[&pin], NOW, None)
            .expect("verifies");

        assert_eq!(
            verified.announcements.len(),
            1,
            "the operator's text is not the unsafe part"
        );
        assert_eq!(
            verified.announcements[0].cta, None,
            "a signature proves who wrote a URL, never that it is safe to click"
        );
    }

    #[test]
    fn the_rendered_table_escapes_operator_text_rather_than_splicing_it() {
        let verified = Verified {
            announcements: vec![DisplayAnnouncement {
                id: "a1".to_owned(),
                headline: "quotes \" and \\ survive".to_owned(),
                body: "body".to_owned(),
                level: "info",
                cta: None,
                voucher_campaign_id: None,
            }],
            active_until: 1_800_003_600,
        };

        assert_eq!(
            serde_json::to_string(&verified).expect("serialize"),
            r#"{"announcements":[{"id":"a1","headline":"quotes \" and \\ survive","body":"body","level":"info","cta":null,"voucher_campaign_id":null}],"active_until":1800003600}"#
        );
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

    #[test]
    fn freeing_a_null_table_is_a_no_op() {
        // SAFETY: null is the documented no-op input.
        unsafe { warren_announcements_free(std::ptr::null_mut()) };
    }
}
