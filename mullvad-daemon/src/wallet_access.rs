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
    ///
    /// Answers whether this call is what claimed the ownership, so a caller
    /// that shows different things to the owner can refresh what it already
    /// published.
    pub fn authorize(&self, peer: Option<PeerCredentials>) -> Result<Access, WalletAccessError> {
        let Some(peer) = peer else {
            // No credentials available (e.g. Windows named pipe): gated elsewhere.
            return Ok(Access::AlreadyHeld);
        };
        if peer.uid == 0 {
            return Ok(Access::AlreadyHeld);
        }
        if self.group_restricted.load(Ordering::Relaxed) {
            // Kernel already enforced group membership at connect time.
            return Ok(Access::AlreadyHeld);
        }
        let mut owner = self.owner_uid.lock().unwrap();
        match *owner {
            None => {
                *owner = Some(peer.uid);
                Ok(Access::JustClaimed)
            }
            Some(u) if u == peer.uid => Ok(Access::AlreadyHeld),
            Some(_) => Err(WalletAccessError::DeniedDifferentUser { uid: peer.uid }),
        }
    }

    /// Whether `peer` may be shown wallet-grade secrets, WITHOUT claiming
    /// anything for it.
    ///
    /// [`Self::authorize`] takes ownership on first use, which is right for a
    /// call that asks for a secret and wrong for a call that merely carries
    /// one alongside ordinary state: a hostile local process racing the GUI to
    /// a status read would inherit the mnemonic with it. So an unclaimed
    /// daemon answers `false` here, and the secret joins the ordinary state
    /// from the moment the owner is known.
    #[must_use]
    pub fn may_see_secrets(&self, peer: Option<PeerCredentials>) -> bool {
        let Some(peer) = peer else {
            return true;
        };
        if peer.uid == 0 || self.group_restricted.load(Ordering::Relaxed) {
            return true;
        }
        *self.owner_uid.lock().unwrap() == Some(peer.uid)
    }
}

/// Whether an authorized call is the one that took the ownership.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    /// The peer already owned wallet access, or the socket is gated by the
    /// kernel and ownership does not apply.
    AlreadyHeld,
    /// This call claimed the ownership: nothing was withheld from this peer
    /// before it, and things may be shown to it now.
    JustClaimed,
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

    /// Reading state must never be what makes a caller the wallet owner: the
    /// claim belongs to a call that asks for a secret outright.
    #[test]
    fn peeking_never_claims_the_ownership() {
        let ac = WalletAccessControl::new();
        ac.set_socket_security(SocketSecurity::WorldAccessible);

        assert!(
            !ac.may_see_secrets(creds(1000)),
            "nobody owns the wallet yet"
        );
        assert_eq!(ac.authorize(creds(1001)), Ok(Access::JustClaimed));
        assert!(
            !ac.may_see_secrets(creds(1000)),
            "the peek must not have made 1000 the owner"
        );
        assert!(ac.may_see_secrets(creds(1001)));
    }

    #[test]
    fn root_and_a_kernel_gated_socket_always_see_secrets() {
        let ac = WalletAccessControl::new();
        ac.set_socket_security(SocketSecurity::WorldAccessible);
        assert!(ac.may_see_secrets(creds(0)));
        assert!(ac.may_see_secrets(None), "no credentials: gated elsewhere");

        ac.set_socket_security(SocketSecurity::GroupRestricted);
        assert!(ac.may_see_secrets(creds(1000)));
    }

    /// The claim is reported once, so a caller that publishes different
    /// things to the owner knows exactly when to publish again.
    #[test]
    fn the_ownership_is_reported_as_claimed_once() {
        let ac = WalletAccessControl::new();
        ac.set_socket_security(SocketSecurity::WorldAccessible);

        assert_eq!(ac.authorize(creds(1000)), Ok(Access::JustClaimed));
        assert_eq!(ac.authorize(creds(1000)), Ok(Access::AlreadyHeld));
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
