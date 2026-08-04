//! Thin app-side wrapper around the SDK's `warren_api::WarrenApiClient`.
//!
//! `WarrenApiClient<T>` owns a `warren_identity::WarrenIdentity`, which only
//! builds from a pre-HKDF BIP39 seed (`WarrenIdentity::from_seed` /
//! `from_mnemonic`) - there is no constructor from an already-derived
//! `ed25519_dalek::SigningKey`. The daemon's shared identity handle
//! (`WarrenAuthSigner::shared`, used by the tunnel and the legacy REST
//! signer) holds exactly that: an already-derived `SigningKey`. Re-deriving
//! a `WarrenIdentity` from its bytes via `from_seed` would run the HKDF step
//! a second time and silently mint a *different* identity.
//!
//! So this module keeps its own shared, hot-swappable BIP39 seed (the
//! pre-derivation input) and rebuilds a fresh `WarrenIdentity` from it on
//! every call: cheap (one HKDF-SHA256 expand, no I/O, no network) and always
//! in sync with the daemon-wide identity because both are re-derived from
//! the same on-disk mnemonic at the same hot-swap events (boot, create /
//! restore via `on_set_warren_mnemonic`, clear via `on_logout_account`; see
//! `WarrenIdentityManager`).

use std::sync::{Arc, RwLock};

use warren_api::{
    ClientError, IncidentExitDownRequest, IncidentPubkeyMismatchRequest, RegisterAccountRequest,
    RegisterAccountResponse, SubscriptionResponse, WarrenApiClient,
};
use warren_identity::WarrenIdentity;
use zeroize::Zeroizing;

use crate::warren_api_transport::WarrenApiTransport;

/// Shared, hot-swappable BIP39 seed backing a Warren identity used only by
/// the SDK-facing clients in this module. `None` at the `Option` level (the
/// call sites carry `Option<SharedWarrenSeed>`) means "no mnemonic
/// bootstrapped yet"; [`sentinel_seed`] is the in-place "logged out"
/// value that `WarrenIdentityManager::clear` swaps BOTH identity views
/// to: a fixed, publicly known identity that must never hold a real
/// subscription (warren-api rejects it outright), so a request that
/// fires after logout fails safely server-side instead of
/// authenticating as the just-erased account.
pub(crate) type SharedWarrenSeed = Arc<RwLock<Zeroizing<[u8; 32]>>>;

/// The "logged out" sentinel seed. See [`SharedWarrenSeed`].
#[must_use]
pub(crate) fn sentinel_seed() -> Zeroizing<[u8; 32]> {
    Zeroizing::new([0u8; 32])
}

fn snapshot_identity(seed: &SharedWarrenSeed) -> WarrenIdentity {
    let guard = seed.read().expect("warren seed RwLock poisoned");
    WarrenIdentity::from_seed(&guard)
}

/// Signed HTTP client for warren-api built around a shared, hot-swappable
/// seed. Every call snapshots the current seed into a fresh
/// `warren_api::WarrenApiClient` (cheap), so a create/restore/logout that
/// swaps the seed takes effect on the very next call without rebuilding
/// this wrapper - preserving the hot-swap contract
/// `WarrenRemoteAccountBackend` and the daemon's incident reporters rely on.
#[derive(Clone)]
pub(crate) struct SharedWarrenApiClient {
    api_base: String,
    seed: SharedWarrenSeed,
    /// Shared, so every `WarrenApiClient` rebuilt by this wrapper reuses the
    /// same connection pool instead of paying a fresh TLS warmup per call.
    transport: WarrenApiTransport,
}

impl SharedWarrenApiClient {
    #[must_use]
    pub fn new(api_base: String, seed: SharedWarrenSeed) -> Self {
        Self {
            api_base,
            seed,
            transport: WarrenApiTransport::new(),
        }
    }

    /// Warren SS58 address of the identity currently backing this client.
    #[must_use]
    pub fn address(&self) -> String {
        snapshot_identity(&self.seed).address()
    }

    /// Same as [`Self::new`], reusing a transport the caller already holds, so
    /// its connection pool is not thrown away with the wrapper.
    #[must_use]
    pub fn with_transport(
        api_base: String,
        seed: SharedWarrenSeed,
        transport: WarrenApiTransport,
    ) -> Self {
        Self {
            api_base,
            seed,
            transport,
        }
    }

    fn client(&self) -> WarrenApiClient<WarrenApiTransport> {
        WarrenApiClient::new(
            self.api_base.clone(),
            snapshot_identity(&self.seed),
            self.transport.clone(),
        )
    }

    /// Signed `GET /v1/subscription`.
    pub async fn get_subscription(&self) -> Result<SubscriptionResponse, ClientError> {
        self.client().subscription().await
    }

    /// Signed `DELETE /v1/account`.
    pub async fn delete_account(&self) -> Result<(), ClientError> {
        self.client().delete_account().await
    }

    /// Unsigned `GET /v1/checkout/{pending_id}/voucher`.
    pub async fn pull_pending_voucher(
        &self,
        pending_id: &str,
    ) -> Result<Option<String>, ClientError> {
        self.client().pull_pending_voucher(pending_id).await
    }

    /// Unsigned `POST /v1/register`.
    pub async fn register_with_voucher(
        &self,
        req: &RegisterAccountRequest,
    ) -> Result<RegisterAccountResponse, ClientError> {
        self.client().register(req).await
    }

    /// Signed `POST /v1/incidents/exit-down`.
    pub async fn report_exit_down(&self, req: &IncidentExitDownRequest) -> Result<(), ClientError> {
        self.client().report_exit_down(req).await
    }

    /// Signed `POST /v1/incidents/pubkey-mismatch`.
    pub async fn report_pubkey_mismatch(
        &self,
        req: &IncidentPubkeyMismatchRequest,
    ) -> Result<(), ClientError> {
        self.client().report_pubkey_mismatch(req).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seed_handle(seed: [u8; 32]) -> SharedWarrenSeed {
        Arc::new(RwLock::new(Zeroizing::new(seed)))
    }

    #[test]
    fn address_matches_the_seed_derived_identity() {
        let seed = seed_handle([7u8; 32]);
        let client = SharedWarrenApiClient::new("https://api.example.test".to_owned(), seed);
        assert_eq!(
            client.address(),
            WarrenIdentity::from_seed(&[7u8; 32]).address(),
            "the reported address must be derived from the current seed"
        );
    }

    #[test]
    fn address_reflects_a_seed_hot_swap() {
        // Regression for the hot-swap contract: writing through the shared
        // seed handle must change what the NEXT call to `address()` (and
        // hence every subsequent signed request) reports, without
        // rebuilding `SharedWarrenApiClient`.
        let seed = seed_handle([1u8; 32]);
        let client =
            SharedWarrenApiClient::new("https://api.example.test".to_owned(), seed.clone());
        let before = client.address();

        *seed.write().unwrap() = Zeroizing::new([2u8; 32]);

        let after = client.address();
        assert_ne!(before, after, "address must change after a seed hot-swap");
        assert_eq!(after, WarrenIdentity::from_seed(&[2u8; 32]).address());
    }

    #[test]
    fn sentinel_seed_is_all_zero() {
        assert_eq!(*sentinel_seed(), [0u8; 32]);
    }
}
