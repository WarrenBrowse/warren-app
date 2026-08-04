use crate::warren_identity::WarrenIdentity;
use serde::{Deserialize, Serialize};

/// Contains the account login state.
///
/// The `LoggedIn` variant carries a [`WarrenIdentity`] (the Ed25519
/// wallet pubkey). The gRPC proto carries the SS58 pubkey address in its
/// `proto::AccountAndDevice` `account_number: String` field.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceState {
    LoggedIn(WarrenIdentity),
    LoggedOut,
    Revoked,
}

impl DeviceState {
    /// Returns the active Warren identity (wallet pubkey) if the
    /// daemon is currently logged in to a valid account.
    pub fn logged_in(self) -> Option<WarrenIdentity> {
        match self {
            DeviceState::LoggedIn(client) => Some(client),
            _ => None,
        }
    }

    pub const fn is_logged_in(&self) -> bool {
        matches!(self, Self::LoggedIn(_))
    }
}

/// Reason why a [DeviceEvent] was emitted.
#[derive(Clone, Debug, Serialize)]
pub enum DeviceEventCause {
    /// Logged in on a new account.
    LoggedIn,
    /// The account was logged out due to user (or daemon) action.
    LoggedOut,
    /// The account was revoked because it was not found remotely.
    Revoked,
}

/// Emitted when logging in or out of an account.
#[derive(Clone, Debug, Serialize)]
pub struct DeviceEvent {
    pub cause: DeviceEventCause,
    pub new_state: DeviceState,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::warren_pubkey::WarrenPubKey;
    use std::str::FromStr;

    fn fixture_pubkey() -> WarrenPubKey {
        // Warren SS58 address of the all-zero 32-byte pubkey (prefix 13295).
        WarrenPubKey::from_str("wb7kgy8FF4rx4tamkksPfoymeeeZVXLrnSjbBxCun3XhP9DnB")
            .expect("fixture SS58 valid")
    }

    #[test]
    fn logged_in_returns_warren_identity_with_pubkey() {
        // `DeviceState::LoggedIn` carries a `WarrenIdentity`;
        // `.logged_in()` therefore returns this identity. The test
        // guarantees that the pubkey survives pattern matching.
        let pk = fixture_pubkey();
        let identity = WarrenIdentity::new(pk.clone());
        let state = DeviceState::LoggedIn(identity);
        let extracted = state.logged_in().expect("must be Some after LoggedIn");
        assert_eq!(extracted.pubkey, pk);
    }

    #[test]
    fn logged_in_returns_none_for_logged_out_or_revoked() {
        assert!(DeviceState::LoggedOut.logged_in().is_none());
        assert!(DeviceState::Revoked.logged_in().is_none());
    }

    #[test]
    fn is_logged_in_matches_only_loggedin_variant() {
        let identity = WarrenIdentity::new(fixture_pubkey());
        assert!(DeviceState::LoggedIn(identity).is_logged_in());
        assert!(!DeviceState::LoggedOut.is_logged_in());
        assert!(!DeviceState::Revoked.is_logged_in());
    }
}
