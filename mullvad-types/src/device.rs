use crate::account::AccountNumber;
use crate::warren_identity::WarrenIdentity;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use talpid_types::net::wireguard::PublicKey;

/// UUID for a device.
pub type DeviceId = String;

/// Human-readable device identifier.
pub type DeviceName = String;

/// Contains data for a device returned by the API.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Device {
    pub id: DeviceId,
    pub name: DeviceName,
    pub pubkey: PublicKey,
    pub hijack_dns: bool,
    pub created: DateTime<Utc>,
}

impl Device {
    /// Return name with each word capitalized: "Happy Seagull" instead of "happy seagull"
    pub fn pretty_name(&self) -> String {
        self.name
            .split_whitespace()
            .map(|word| {
                let mut chars = word.chars();
                match chars.next() {
                    None => String::new(),
                    Some(c) => c.to_uppercase().chain(chars).collect(),
                }
            })
            .collect::<Vec<String>>()
            .join(" ")
    }

    pub fn eq_id(&self, other: &Device) -> bool {
        self.id == other.id
    }
}

/// Contains a device state.
///
/// Warren fork — Phase 2.B.3 V6.a : le variant `LoggedIn` porte
/// désormais une [`WarrenIdentity`] (pubkey Ed25519 + device) au lieu
/// de l'historique [`AccountAndDevice`] (account_number String +
/// device). La struct [`AccountAndDevice`] reste définie ci-dessous
/// pour transition (gRPC proto + `From` impl côté daemon) ; suppression
/// prévue en V6.c.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceState {
    LoggedIn(WarrenIdentity),
    LoggedOut,
    Revoked,
}

impl DeviceState {
    /// Returns the active Warren identity (pubkey + device) if the
    /// device is currently logged in to a valid account.
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

/// A [Device] and its associated account number.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AccountAndDevice {
    #[serde(alias = "account_token")]
    pub account_number: AccountNumber,
    pub device: Device,
}

impl AccountAndDevice {
    pub fn new(account_number: AccountNumber, device: Device) -> Self {
        Self {
            account_number,
            device,
        }
    }
}

/// Reason why a [DeviceEvent] was emitted.
#[derive(Clone, Debug, Serialize)]
pub enum DeviceEventCause {
    /// Logged in on a new device.
    LoggedIn,
    /// The device was removed due to user (or daemon) action.
    LoggedOut,
    /// Device was removed because it was not found remotely.
    Revoked,
    /// The device was updated, but not its key.
    Updated,
    /// The key was rotated.
    RotatedKey,
}

/// Emitted when logging in or out of an account, or when the device changes.
#[derive(Clone, Debug, Serialize)]
pub struct DeviceEvent {
    pub cause: DeviceEventCause,
    pub new_state: DeviceState,
}

/// Emitted when a device is removed using the `RemoveDevice` RPC.
/// This is not sent by a normal logout or when it is revoked remotely.
#[derive(Clone, Debug, Serialize)]
pub struct RemoveDeviceEvent {
    pub account_number: AccountNumber,
    pub new_devices: Vec<Device>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::warren_pubkey::WarrenPubKey;
    use chrono::TimeZone;
    use std::str::FromStr;

    fn fixture_device() -> Device {
        Device {
            id: "id-fixture".to_owned(),
            name: "happy seagull".to_owned(),
            pubkey: PublicKey::from([0u8; 32]),
            hijack_dns: false,
            created: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
        }
    }

    fn fixture_pubkey() -> WarrenPubKey {
        WarrenPubKey::from_str("abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789")
            .expect("fixture hex valid")
    }

    #[test]
    fn logged_in_returns_warren_identity_with_pubkey() {
        // Phase 2.B.3 V6.a — `DeviceState::LoggedIn` porte une
        // `WarrenIdentity` ; `.logged_in()` retourne donc cette
        // identité et non plus un `AccountAndDevice`. Test garantit
        // que la pubkey survie le pattern matching.
        let pk = fixture_pubkey();
        let identity = WarrenIdentity::new(pk.clone(), fixture_device());
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
        let identity = WarrenIdentity::new(fixture_pubkey(), fixture_device());
        assert!(DeviceState::LoggedIn(identity).is_logged_in());
        assert!(!DeviceState::LoggedOut.is_logged_in());
        assert!(!DeviceState::Revoked.is_logged_in());
    }
}
