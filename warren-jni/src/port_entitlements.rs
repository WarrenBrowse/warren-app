//! Port-forwarding entitlements (warren-core doc 99, doc 105) for the Android
//! tunnel. The mint itself is `warren_standing::entitlements`, shared with the
//! desktop daemon and iOS; this module only wires it to the Android wallet,
//! transport and standing.
//!
//! Android runs the single preferred-port model, so its one rule draws slot 0.

pub(crate) use warren_standing::entitlements::{CredentialSource, await_first_credential};

#[cfg(all(target_os = "android", feature = "tunnel"))]
pub(crate) use android::provider_for;

#[cfg(all(target_os = "android", feature = "tunnel"))]
mod android {
    use std::sync::{Arc, OnceLock};

    use ed25519_dalek::SigningKey;
    use warren_api::WarrenApiClient;
    use warren_identity::WarrenIdentity;
    use warren_standing::entitlements::EntitlementMint;

    use super::CredentialSource;
    use crate::protected_transport::ProtectedTransport;

    /// Android runs the single preferred-port model (one rule, one forwarded
    /// port), so every session draws the first slot of the batch. The desktop
    /// multi-rule editor is what makes slot assignment dynamic there.
    const ANDROID_RULE_SLOT: usize = 0;

    /// Process-lived mint registry: survives connect/disconnect cycles so the
    /// refresh cadence stays decoupled from session timing, and so a redial
    /// re-presents the credential the exit already spent for this port. The
    /// transport is the VpnService-protected one, for the same reason as the
    /// token mint: an unprotected socket loses the tunnel bring-up race.
    static MINT: OnceLock<EntitlementMint<ProtectedTransport>> = OnceLock::new();

    fn now_unix_secs() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }

    /// The entitlement source for `signing_key`'s wallet against the compiled
    /// product API. The minting identity is built from the SAME Ed25519 key
    /// the tunnel handshake signs with, so the minting wallet is bit-for-bit
    /// the subscribed wallet.
    pub(crate) fn provider_for(signing_key: SigningKey) -> CredentialSource {
        let mint = MINT.get_or_init(|| {
            EntitlementMint::new(Arc::new(now_unix_secs)).with_ban_sink(Arc::new(
                |wallet, error| {
                    crate::standing::store().on_refresh_error(wallet, error, now_unix_secs())
                },
            ))
        });
        let wallet_pubkey = signing_key.verifying_key().to_bytes();
        mint.credential_source(wallet_pubkey, ANDROID_RULE_SLOT, move || {
            WarrenApiClient::new(
                crate::product::PRODUCT_API_URL.to_owned(),
                WarrenIdentity::from_signing_key(signing_key),
                ProtectedTransport::new(),
            )
        })
    }
}
