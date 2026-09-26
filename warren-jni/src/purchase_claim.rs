//! The purchase claim `WarrenJni.redeemVoucher` recognizes: the wpid the app
//! opened the checkout with, followed by the pull secret warren-api hands the
//! voucher out against. Host-tested; the redeem that consumes it is
//! Android-gated in `android_jni`.

use zeroize::Zeroizing;

/// A purchase to collect rather than a voucher to redeem.
pub(crate) struct PurchaseClaim {
    pub(crate) wpid: String,
    /// Bearer material for a paid voucher: wiped on drop.
    pub(crate) pull_secret: Zeroizing<String>,
}

/// Detect the claim shape: exactly 96 ASCII hex chars (after trimming), the
/// 32-hex wpid followed by the 64-hex pull secret, lowercased. A voucher (16
/// Crockford-32 characters) can never take that shape, so anything else is a
/// regular voucher secret. Mirrors the desktop daemon's `as_purchase_claim`.
pub(crate) fn parse(input: &str) -> Option<PurchaseClaim> {
    let trimmed = input.trim();
    if trimmed.len() != 96 || !trimmed.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let lower = Zeroizing::new(trimmed.to_ascii_lowercase());
    let (wpid, pull_secret) = lower.split_at(32);
    Some(PurchaseClaim {
        wpid: wpid.to_owned(),
        pull_secret: Zeroizing::new(pull_secret.to_owned()),
    })
}

/// What a failed `POST /v1/register` does to a redemption, and to the pulled
/// secret the pull left as the only copy of a paid voucher. Mirrors the
/// desktop daemon's `submit_voucher`: only a verdict on the voucher itself
/// drops the secret.
#[derive(Debug)]
pub(crate) enum RegisterFailure {
    /// A transport failure or a server error: retry, keep the secret.
    Transient,
    /// The wallet is banned (warren-core doc 105 section 5.3): the refusal
    /// consumed nothing, so the secret is kept for after the ban, and a retry
    /// cannot change the answer.
    Banned(warren_standing::Ban),
    /// The voucher is unknown, spent, cancelled or expired: the secret is
    /// worth nothing any more.
    VoucherDead,
    /// Any other refusal: final for this attempt, the secret is kept.
    Refused,
}

pub(crate) fn classify_register_failure(error: &warren_api::ClientError) -> RegisterFailure {
    if let Some(ban) = warren_standing::Ban::from_client_error(error) {
        return RegisterFailure::Banned(ban);
    }
    match error {
        warren_api::ClientError::ServerStatus { status, body } if *status < 500 => {
            // The statuses the server gives a voucher verdict, told apart from
            // a malformed request or a throttle by naming the voucher.
            if matches!(status, 400 | 409 | 410) && body.contains("voucher") {
                RegisterFailure::VoucherDead
            } else {
                RegisterFailure::Refused
            }
        }
        _ => RegisterFailure::Transient,
    }
}

/// Voucher secrets pulled for a purchase and not redeemed yet, keyed by wpid.
/// The server hands a voucher out once, so a caller that lost the answer (a
/// register that failed, Kotlin dying before it sealed the voucher) finds it
/// here again for the life of the process.
/// Each entry keeps the pull secret that collected its voucher: the wpid
/// names the purchase in URLs and proves nothing, so the copy is handed back
/// against the same secret the server asked for. Both are paid bearer
/// secrets, wiped when they leave.
pub(crate) struct PulledVouchers(parking_lot::Mutex<std::collections::BTreeMap<String, Kept>>);

/// The pull secret that collected a voucher, and the voucher.
type Kept = (Zeroizing<String>, Zeroizing<String>);

impl PulledVouchers {
    pub(crate) const fn new() -> Self {
        Self(parking_lot::Mutex::new(std::collections::BTreeMap::new()))
    }

    /// The voucher kept for `claim`, if its pull secret is the one that
    /// collected it.
    pub(crate) fn get(&self, claim: &PurchaseClaim) -> Option<Zeroizing<String>> {
        let kept = self.0.lock();
        let (pull_secret, voucher) = kept.get(&claim.wpid)?;
        same_secret(pull_secret, &claim.pull_secret).then(|| voucher.clone())
    }

    pub(crate) fn keep(&self, claim: &PurchaseClaim, voucher: &str) {
        self.0.lock().insert(
            claim.wpid.clone(),
            (
                claim.pull_secret.clone(),
                Zeroizing::new(voucher.to_owned()),
            ),
        );
    }

    /// Drops every copy of `voucher` once it is redeemed or dead, whether it
    /// was redeemed through its claim or, sealed by Kotlin, as itself.
    pub(crate) fn forget(&self, voucher: &str) {
        self.0
            .lock()
            .retain(|_, (_, kept)| kept.as_str() != voucher);
    }
}

/// Compares two secrets in time independent of where they first differ.
fn same_secret(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0u8, |diff, (x, y)| diff | (x ^ y))
            == 0
}

#[cfg(test)]
mod tests {
    use super::{PulledVouchers, RegisterFailure, classify_register_failure, parse};

    fn claim(wpid_char: char, secret_char: char) -> super::PurchaseClaim {
        parse(&format!(
            "{}{}",
            wpid_char.to_string().repeat(32),
            secret_char.to_string().repeat(64)
        ))
        .expect("96 hex chars are a claim")
    }

    #[test]
    fn a_pulled_voucher_is_found_again_under_its_purchase() {
        let pulled = PulledVouchers::new();

        pulled.keep(&claim('a', '1'), "XXXX-YYYY-ZZZZ-WWWW");

        assert_eq!(
            pulled.get(&claim('a', '1')).as_deref().map(String::as_str),
            Some("XXXX-YYYY-ZZZZ-WWWW")
        );
        assert!(pulled.get(&claim('b', '1')).is_none());
    }

    #[test]
    fn the_kept_copy_answers_only_the_purchase_s_own_pull_secret() {
        // The wpid travels in URLs and proves nothing (warren-core doc 35).
        let pulled = PulledVouchers::new();

        pulled.keep(&claim('a', '1'), "XXXX-YYYY-ZZZZ-WWWW");

        assert!(pulled.get(&claim('a', '2')).is_none());
    }

    #[test]
    fn a_redeemed_or_dead_voucher_leaves_every_purchase_it_was_kept_under() {
        let pulled = PulledVouchers::new();
        pulled.keep(&claim('a', '1'), "XXXX-YYYY-ZZZZ-WWWW");
        pulled.keep(&claim('b', '1'), "QQQQ-RRRR-SSSS-TTTT");

        pulled.forget("XXXX-YYYY-ZZZZ-WWWW");

        assert!(pulled.get(&claim('a', '1')).is_none());
        assert_eq!(
            pulled.get(&claim('b', '1')).as_deref().map(String::as_str),
            Some("QQQQ-RRRR-SSSS-TTTT")
        );
    }
    use warren_api::{BanReasonCode, ClientError};

    fn refused(status: u16, body: &str) -> ClientError {
        ClientError::ServerStatus {
            status,
            body: body.to_owned(),
        }
    }

    #[test]
    fn a_ban_refusal_stops_the_redemption_and_keeps_the_voucher() {
        let failure = classify_register_failure(&ClientError::Banned {
            reason_code: BanReasonCode::PortForwardingAbuse,
            lapses_at_unix_secs: None,
        });

        match failure {
            RegisterFailure::Banned(ban) => {
                assert_eq!(ban.reason, BanReasonCode::PortForwardingAbuse);
                assert_eq!(ban.lapses_at_unix_secs, None);
            }
            other => panic!("expected Banned, got {other:?}"),
        }
    }

    #[test]
    fn only_a_verdict_on_the_voucher_itself_kills_it() {
        for (status, body) in [
            (400, r#"{"error":"voucher unknown or invalid"}"#),
            (409, r#"{"error":"voucher already redeemed"}"#),
            (409, r#"{"error":"voucher redemption limit reached"}"#),
            (410, r#"{"error":"voucher was cancelled by the admin"}"#),
            (410, r#"{"error":"voucher expired"}"#),
        ] {
            assert!(
                matches!(
                    classify_register_failure(&refused(status, body)),
                    RegisterFailure::VoucherDead
                ),
                "{status} {body}"
            );
        }
    }

    #[test]
    fn a_refusal_that_says_nothing_of_the_voucher_keeps_it_without_retrying() {
        for (status, body) in [
            (429, ""),
            (403, r#"{"error":"forbidden"}"#),
            (409, r#"{"error":"pubkey already registered"}"#),
        ] {
            assert!(
                matches!(
                    classify_register_failure(&refused(status, body)),
                    RegisterFailure::Refused
                ),
                "{status} {body}"
            );
        }
    }

    #[test]
    fn a_server_error_or_a_transport_failure_is_retried() {
        assert!(matches!(
            classify_register_failure(&refused(503, "")),
            RegisterFailure::Transient
        ));
        assert!(matches!(
            classify_register_failure(&ClientError::BadClock),
            RegisterFailure::Transient
        ));
    }

    const SECRET: &str = "abababababababababababababababababababababababababababababababab";

    #[test]
    fn a_claim_is_96_hex_chars_in_any_case_and_comes_out_lowercased() {
        let claim = parse(&format!(
            "  0123456789ABCDEF0123456789abcdef{}\n",
            SECRET.to_uppercase()
        ))
        .expect("96 hex chars are a claim");

        assert_eq!(claim.wpid, "0123456789abcdef0123456789abcdef");
        assert_eq!(claim.pull_secret.as_str(), SECRET);
    }

    #[test]
    fn a_bare_wpid_or_a_voucher_is_not_a_claim() {
        // A wpid alone carries no pull secret, so it collects nothing.
        assert!(parse("0123456789abcdef0123456789abcdef").is_none());
        assert!(parse("ABCD-EFGH-JKMN-PQRS").is_none());
        assert!(parse("ABCDEFGHJKMNPQRS").is_none());
        assert!(parse("").is_none());
        assert!(parse(&format!("0123456789abcdef0123456789abcdeg{SECRET}")).is_none());
    }
}
