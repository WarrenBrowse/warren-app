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

#[cfg(test)]
mod tests {
    use super::{RegisterFailure, classify_register_failure, parse};
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
