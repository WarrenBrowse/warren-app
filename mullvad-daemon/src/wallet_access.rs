//! Authorization for wallet/secret management RPCs.
//!
//! The BIP39 mnemonic is a wallet-grade secret. The management socket is
//! reachable by local processes, so secret-returning and identity-wiping
//! RPCs (`get_warren_mnemonic`, `set_warren_mnemonic`, the destructive
//! sign-out wipe) are gated here against the calling process' Unix
//! credentials (`SO_PEERCRED`, captured by the management interface).
//!
//! Policy:
//! - **root** (uid 0) is always allowed (the daemon itself / admin).
//! - When the socket is **group-restricted** (operator opted in via
//!   `WARREN_MANAGEMENT_SOCKET_GROUP`), the kernel already enforced that
//!   only authorized users (root + that group) could connect, so any peer
//!   that got this far is allowed.
//! - When the socket is **world-accessible** (the default, matching
//!   upstream Mullvad's local-users-are-trusted threat model), we apply
//!   trust-on-first-use: the first non-root uid to touch a wallet RPC
//!   becomes the owner, and a different uid is denied. This closes the
//!   steady-state multi-user exfiltration (another user reading the
//!   desktop user's seed) without breaking the single-user GUI.
//! - **Unknown credentials** (`None`, e.g. the Windows named pipe) are
//!   allowed; that channel is gated by its DACL + the desktop's
//!   admin-ownership check instead.
//!
//! Residual (world-accessible mode only): on a multi-user box a hostile
//! process that races the GUI to be the very first wallet caller can claim
//! ownership. Hardened multi-user deployments should set
//! `WARREN_MANAGEMENT_SOCKET_GROUP` to a dedicated group, which makes the
//! kernel-enforced group branch authoritative and bypasses TOFU entirely.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use mullvad_management_interface::{PeerCredentials, SocketSecurity};

/// Process-lifetime authorization state for wallet/secret RPCs.
pub struct WalletAccessControl {
    /// Whether the management socket is restricted to root + a Unix group.
    group_restricted: AtomicBool,
    /// Trust-on-first-use owner uid, only consulted in world-accessible mode.
    owner_uid: Mutex<Option<u32>>,
}

/// Reason a wallet/secret RPC was refused.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum WalletAccessError {
    #[error(
        "wallet access denied for uid {uid}: this account's secrets belong to another local user"
    )]
    DeniedDifferentUser { uid: u32 },
}

impl WalletAccessControl {
    pub fn new() -> Self {
        Self {
            group_restricted: AtomicBool::new(false),
            owner_uid: Mutex::new(None),
        }
    }

    /// Record the access-control mode the socket ended up in once it is bound.
    pub fn set_socket_security(&self, security: SocketSecurity) {
        self.group_restricted.store(
            matches!(security, SocketSecurity::GroupRestricted),
            Ordering::Relaxed,
        );
    }

    /// Authorize a wallet/secret operation from a peer. `peer` is the
    /// connect-info captured by the management interface (`None` when the
    /// platform cannot supply credentials).
    pub fn authorize(&self, peer: Option<PeerCredentials>) -> Result<(), WalletAccessError> {
        let Some(peer) = peer else {
            // No credentials available (e.g. Windows named pipe): gated elsewhere.
            return Ok(());
        };
        if peer.uid == 0 {
            return Ok(());
        }
        if self.group_restricted.load(Ordering::Relaxed) {
            // Kernel already enforced group membership at connect time.
            return Ok(());
        }
        let mut owner = self.owner_uid.lock().unwrap();
        match *owner {
            None => {
                *owner = Some(peer.uid);
                Ok(())
            }
            Some(u) if u == peer.uid => Ok(()),
            Some(_) => Err(WalletAccessError::DeniedDifferentUser { uid: peer.uid }),
        }
    }
}

impl Default for WalletAccessControl {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn creds(uid: u32) -> Option<PeerCredentials> {
        Some(PeerCredentials {
            uid,
            gid: uid,
            pid: Some(1234),
        })
    }

    #[test]
    fn root_is_always_allowed() {
        let ac = WalletAccessControl::new();
        ac.set_socket_security(SocketSecurity::WorldAccessible);
        assert!(ac.authorize(creds(0)).is_ok());
        ac.set_socket_security(SocketSecurity::GroupRestricted);
        assert!(ac.authorize(creds(0)).is_ok());
    }

    #[test]
    fn unknown_credentials_allowed() {
        let ac = WalletAccessControl::new();
        ac.set_socket_security(SocketSecurity::WorldAccessible);
        assert!(ac.authorize(None).is_ok());
    }

    #[test]
    fn group_restricted_allows_any_connected_peer() {
        let ac = WalletAccessControl::new();
        ac.set_socket_security(SocketSecurity::GroupRestricted);
        // Two different non-root users both got through the group gate.
        assert!(ac.authorize(creds(1000)).is_ok());
        assert!(ac.authorize(creds(1001)).is_ok());
    }

    #[test]
    fn world_accessible_tofu_binds_first_owner() {
        let ac = WalletAccessControl::new();
        ac.set_socket_security(SocketSecurity::WorldAccessible);
        // First non-root caller becomes the owner.
        assert!(ac.authorize(creds(1000)).is_ok());
        // Same uid keeps working.
        assert!(ac.authorize(creds(1000)).is_ok());
        // A different uid is denied.
        assert_eq!(
            ac.authorize(creds(1001)),
            Err(WalletAccessError::DeniedDifferentUser { uid: 1001 })
        );
    }

    #[test]
    fn world_accessible_root_does_not_claim_ownership() {
        let ac = WalletAccessControl::new();
        ac.set_socket_security(SocketSecurity::WorldAccessible);
        // Root touches a wallet RPC first; it must not become the TOFU owner,
        // otherwise the legitimate desktop user would be locked out.
        assert!(ac.authorize(creds(0)).is_ok());
        assert!(ac.authorize(creds(1000)).is_ok());
        assert!(ac.authorize(creds(1000)).is_ok());
    }
}
