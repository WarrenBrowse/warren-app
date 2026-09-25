//! The networks "Local network sharing" lets through outside the tunnel.

use ipnetwork::IpNetwork;
use talpid_types::net::ALLOWED_LAN_NETS;

use super::Settings;

/// The widest IPv4 prefix accepted, the width of the widest built-in range
/// (10.0.0.0/8).
const MIN_IPV4_PREFIX: u8 = 8;
/// The widest IPv6 prefix accepted, the width of the widest built-in range
/// (fc00::/7).
const MIN_IPV6_PREFIX: u8 = 7;

#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
pub enum LanNetworkError {
    /// The network is wider than any local network, so sharing it would
    /// route a large part of the Internet around the tunnel.
    #[error("{0} is too broad to be a local network")]
    TooBroad(IpNetwork),
}

/// Normalises a user-supplied list of networks to share: host bits are
/// cleared and duplicates dropped, keeping the first position of each.
///
/// # Errors
///
/// [`LanNetworkError::TooBroad`] for the first network wider than a /8
/// (IPv4) or a /7 (IPv6).
pub fn validate_lan_networks(networks: Vec<IpNetwork>) -> Result<Vec<IpNetwork>, LanNetworkError> {
    let mut validated: Vec<IpNetwork> = Vec::with_capacity(networks.len());
    for network in networks {
        let min_prefix = match network {
            IpNetwork::V4(_) => MIN_IPV4_PREFIX,
            IpNetwork::V6(_) => MIN_IPV6_PREFIX,
        };
        if network.prefix() < min_prefix {
            return Err(LanNetworkError::TooBroad(network));
        }
        let network = IpNetwork::new(network.network(), network.prefix())
            .expect("the prefix of a parsed network stays valid");
        if !validated.contains(&network) {
            validated.push(network);
        }
    }
    Ok(validated)
}

impl Settings {
    /// The networks shared while `allow_lan` is on: the user's list when
    /// they customised it, the built-in private ranges otherwise.
    pub fn lan_networks(&self) -> Vec<IpNetwork> {
        self.custom_lan_networks
            .clone()
            .unwrap_or_else(|| ALLOWED_LAN_NETS.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Settings;
    use talpid_types::net::ALLOWED_LAN_NETS;

    fn net(s: &str) -> IpNetwork {
        s.parse().expect("valid network literal")
    }

    /// A settings file written before the list was customisable has no key
    /// for it, and must keep sharing exactly the built-in private ranges.
    #[test]
    fn settings_without_the_key_share_the_default_ranges() {
        let json = serde_json::json!({ "allow_lan": true });
        let settings: Settings = serde_json::from_value(json).expect("settings deserialise");
        assert_eq!(settings.custom_lan_networks, None);
        assert_eq!(settings.lan_networks(), ALLOWED_LAN_NETS.to_vec());
    }

    /// Once customised, the user's list replaces the defaults entirely, so a
    /// default range the user removed is no longer shared.
    #[test]
    fn a_custom_list_replaces_the_default_ranges() {
        let custom = vec![net("192.168.0.0/16"), net("400::/7")];
        let settings = Settings {
            custom_lan_networks: Some(custom.clone()),
            ..Default::default()
        };
        assert_eq!(settings.lan_networks(), custom);
    }

    /// The user types an address inside the network they mean; the firewall
    /// wants the network itself.
    #[test]
    fn host_bits_are_cleared() {
        let validated = validate_lan_networks(vec![net("192.168.1.42/24")]).expect("valid");
        assert_eq!(validated, vec![net("192.168.1.0/24")]);
    }

    /// The same network entered twice is shared once, in first-seen order.
    #[test]
    fn duplicates_are_dropped_keeping_the_first_position() {
        let validated =
            validate_lan_networks(vec![net("400::/7"), net("10.0.0.0/8"), net("400::1/7")])
                .expect("valid");
        assert_eq!(validated, vec![net("400::/7"), net("10.0.0.0/8")]);
    }

    /// Anything wider than the widest built-in range would carry a large
    /// part of the Internet outside the tunnel, which is a kill switch the
    /// user switched off by typo, not a local network.
    #[test]
    fn rejects_an_ipv4_network_wider_than_a_slash_8() {
        let error = validate_lan_networks(vec![net("10.0.0.0/8"), net("0.0.0.0/0")])
            .expect_err("a default route is not a local network");
        assert_eq!(error, LanNetworkError::TooBroad(net("0.0.0.0/0")));
    }

    #[test]
    fn rejects_an_ipv6_network_wider_than_a_slash_7() {
        let error = validate_lan_networks(vec![net("8000::/1")])
            .expect_err("half of IPv6 is not a local network");
        assert_eq!(error, LanNetworkError::TooBroad(net("8000::/1")));
    }

    /// The widest accepted prefixes are the ones the defaults already use,
    /// and they cover the Mycelium overlay (400::/7).
    #[test]
    fn accepts_the_widest_allowed_prefixes() {
        let nets = vec![net("100.0.0.0/8"), net("400::/7")];
        assert_eq!(validate_lan_networks(nets.clone()), Ok(nets));
    }

    /// Emptying the list is a legitimate choice: only the multicast and
    /// broadcast ranges stay reachable.
    #[test]
    fn accepts_an_empty_list() {
        assert_eq!(validate_lan_networks(vec![]), Ok(vec![]));
    }
}
