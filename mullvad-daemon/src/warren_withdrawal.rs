//! In-app withdrawal (rétractation) of a purchase this wallet redeemed: the
//! wallet-signed `POST /v1/account/refund/by-wpid` (warren-core
//! `handlers/withdrawal.rs`). The buyer whose app pulled and redeemed the
//! voucher never saw its code, so the signature of the redeeming wallet is the
//! proof of purchase. Neither the wpid nor the refund address is ever logged.

use warren_api::{ClientError, dto::WithdrawalAck};

/// Why a declaration was not recorded, in the terms the GUI acts on.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DeclarationError {
    /// No wallet is logged in, so nothing can sign the declaration.
    #[error("no wallet is logged in")]
    NoWallet,
    /// The purchase id or the refund address is malformed (422, or caught
    /// before the request).
    #[error("the purchase id or the refund address is malformed")]
    Invalid,
    /// This wallet did not redeem the purchase's voucher, or the purchase is
    /// unknown (403).
    #[error("this wallet did not redeem that purchase")]
    NotProven,
    /// A declaration for this purchase is already on file with another
    /// refund address (409); support settles it.
    #[error("a declaration with another refund address is already on file")]
    AlreadyFiled,
    /// The API could not be reached or did not record it; a retry can work.
    #[error("the declaration could not be recorded, retry later")]
    Unavailable,
}

/// Maps the SDK error of a declaration to the verdict the GUI shows. Only the
/// status code is read: the body may echo identity material and is never
/// looked at or logged.
pub fn classify(error: &ClientError) -> DeclarationError {
    match error {
        ClientError::ServerStatus { status: 403, .. } => DeclarationError::NotProven,
        ClientError::ServerStatus { status: 409, .. } => DeclarationError::AlreadyFiled,
        ClientError::ServerStatus { status: 422, .. } => DeclarationError::Invalid,
        _ => DeclarationError::Unavailable,
    }
}

/// A purchase id is the 32 lowercase or uppercase hex characters the app
/// minted for the purchase (`purchase-claim.ts`).
pub fn is_valid_wpid(wpid: &str) -> bool {
    wpid.len() == 32 && wpid.bytes().all(|b| b.is_ascii_hexdigit())
}

/// The refund address the buyer typed, trimmed, `None` when left empty (a
/// card purchase is refunded to the card and needs none).
pub fn normalize_refund_address(typed: Option<&str>) -> Option<String> {
    typed
        .map(str::trim)
        .filter(|address| !address.is_empty())
        .map(str::to_owned)
}

/// Declares the withdrawal of `wpid` through the signed route.
pub async fn declare(
    client: &crate::warren_sdk_client::SharedWarrenApiClient,
    wpid: &str,
    refund_address: Option<&str>,
) -> Result<WithdrawalAck, DeclarationError> {
    if client.wallet().is_none() {
        return Err(DeclarationError::NoWallet);
    }
    if !is_valid_wpid(wpid) {
        return Err(DeclarationError::Invalid);
    }
    let address = normalize_refund_address(refund_address);
    client
        .declare_crypto_withdrawal(wpid, address.as_deref())
        .await
        .map_err(|error| classify(&error))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status(status: u16) -> ClientError {
        ClientError::ServerStatus {
            status,
            body: String::from("never read"),
        }
    }

    #[test]
    fn a_refusal_for_a_purchase_this_wallet_did_not_redeem_is_not_proven() {
        assert_eq!(classify(&status(403)), DeclarationError::NotProven);
    }

    #[test]
    fn a_second_declaration_with_another_address_is_already_filed() {
        assert_eq!(classify(&status(409)), DeclarationError::AlreadyFiled);
    }

    #[test]
    fn a_malformed_declaration_is_invalid() {
        assert_eq!(classify(&status(422)), DeclarationError::Invalid);
    }

    #[test]
    fn any_other_failure_is_worth_a_retry() {
        assert_eq!(classify(&status(503)), DeclarationError::Unavailable);
        assert_eq!(classify(&status(500)), DeclarationError::Unavailable);
    }

    #[test]
    fn a_purchase_id_is_32_hex_characters() {
        assert!(is_valid_wpid("0123456789abcdef0123456789ABCDEF"));
        assert!(!is_valid_wpid("0123456789abcdef0123456789abcde"));
        assert!(!is_valid_wpid("0123456789abcdef0123456789abcdeg"));
        assert!(!is_valid_wpid(""));
    }

    #[test]
    fn an_empty_refund_address_is_no_address() {
        assert_eq!(normalize_refund_address(None), None);
        assert_eq!(normalize_refund_address(Some("   ")), None);
        assert_eq!(
            normalize_refund_address(Some("  bc1qexample  ")),
            Some(String::from("bc1qexample"))
        );
    }
}
