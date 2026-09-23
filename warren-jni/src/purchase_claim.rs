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

#[cfg(test)]
mod tests {
    use super::parse;

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
