//! The networks "Local network sharing" lets through outside the tunnel.

use ipnetwork::{IpNetwork, Ipv6Network};
use std::net::Ipv6Addr;
use talpid_types::net::ALLOWED_LAN_NETS;

use super::Settings;

/// The widest IPv4 prefix accepted, the width of the widest built-in range
/// (10.0.0.0/8).
const MIN_IPV4_PREFIX: u8 = 8;
/// The widest IPv6 prefix accepted, the width of the widest built-in range
/// (fc00::/7). Overlay networks such as Mycelium (400::/7) use the same width.
const MIN_IPV6_PREFIX: u8 = 7;
/// The widest prefix accepted inside global unicast IPv6 (2000::/3), where
/// allocations are made in /16 blocks and wider.
const MIN_GLOBAL_UNICAST_IPV6_PREFIX: u8 = 16;
/// Every network becomes firewall rules, and a policy too large to apply
/// leaves the machine unprotected.
pub const MAX_LAN_NETWORKS: usize = 64;

/// Prefixes that embed IPv4 addresses: NAT64 (well-known and local-use),
/// 6to4, Teredo, IPv4-mapped and IPv4-compatible. Sharing one would let an
/// IPv6-only network reach any IPv4 host around the tunnel.
const IPV4_TRANSLATION_PREFIXES: [(Ipv6Addr, u8); 6] = [
    (Ipv6Addr::new(0x64, 0xff9b, 0, 0, 0, 0, 0, 0), 96),
    (Ipv6Addr::new(0x64, 0xff9b, 1, 0, 0, 0, 0, 0), 48),
    (Ipv6Addr::new(0x2002, 0, 0, 0, 0, 0, 0, 0), 16),
    (Ipv6Addr::new(0x2001, 0, 0, 0, 0, 0, 0, 0), 32),
    (Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0, 0), 96),
    (Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0), 96),
];

const GLOBAL_UNICAST_IPV6: (Ipv6Addr, u8) = (Ipv6Addr::new(0x2000, 0, 0, 0, 0, 0, 0, 0), 3);

#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum LanNetworkError {
    /// The network is wider than any local network, so sharing it would
    /// route a large part of the Internet around the tunnel.
    #[error("{0} is too broad to be a local network")]
    TooBroad(IpNetwork),
    /// The network overlaps a prefix that embeds IPv4 addresses.
    #[error("{0} overlaps a prefix that carries IPv4 traffic")]
    TranslatesIpv4(IpNetwork),
    /// The list is longer than [`MAX_LAN_NETWORKS`].
    #[error("at most {MAX_LAN_NETWORKS} networks can be shared")]
    TooMany,
}

/// Normalises a user-supplied list of networks to share: host bits are
/// cleared and duplicates dropped, keeping the first position of each.
///
/// # Errors
///
/// - [`LanNetworkError::TooMany`] for a list longer than [`MAX_LAN_NETWORKS`].
/// - [`LanNetworkError::TooBroad`] for the first network wider than a /8
///   (IPv4), a /7 (IPv6), or a /16 inside global unicast IPv6.
/// - [`LanNetworkError::TranslatesIpv4`] for the first network overlapping a
///   prefix that embeds IPv4 addresses.
pub fn validate_lan_networks(networks: Vec<IpNetwork>) -> Result<Vec<IpNetwork>, LanNetworkError> {
    if networks.len() > MAX_LAN_NETWORKS {
        return Err(LanNetworkError::TooMany);
    }
    let mut validated: Vec<IpNetwork> = Vec::with_capacity(networks.len());
    for network in networks {
        check_lan_network(network)?;
        let network = IpNetwork::new(network.network(), network.prefix()).unwrap_or(network);
        if !validated.contains(&network) {
            validated.push(network);
        }
    }
    Ok(validated)
}

fn check_lan_network(network: IpNetwork) -> Result<(), LanNetworkError> {
    let IpNetwork::V6(v6) = network else {
        return if network.prefix() < MIN_IPV4_PREFIX {
            Err(LanNetworkError::TooBroad(network))
        } else {
            Ok(())
        };
    };
    if v6.prefix() < MIN_IPV6_PREFIX {
        return Err(LanNetworkError::TooBroad(network));
    }
    if IPV4_TRANSLATION_PREFIXES
        .iter()
        .any(|&prefix| overlaps(v6, prefix))
    {
        return Err(LanNetworkError::TranslatesIpv4(network));
    }
    if overlaps(v6, GLOBAL_UNICAST_IPV6) && v6.prefix() < MIN_GLOBAL_UNICAST_IPV6_PREFIX {
        return Err(LanNetworkError::TooBroad(network));
    }
    Ok(())
}

/// Whether `network` reaches past the private address space: an IPv4 range
/// outside the built-in private ranges and the shared address space
/// (100.64.0.0/10, used by overlay networks), or an IPv6 range inside global
/// unicast. The firewall lets such a range through outside the tunnel on any
/// interface, so a route a hostile network pushes for it carries that
/// traffic in clear.
pub fn is_public_lan_network(network: &IpNetwork) -> bool {
    match network {
        IpNetwork::V4(v4) => {
            let shared_address_space =
                IpNetwork::new(std::net::Ipv4Addr::new(100, 64, 0, 0).into(), 10)
                    .expect("a valid prefix");
            !ALLOWED_LAN_NETS
                .iter()
                .chain(std::iter::once(&shared_address_space))
                .any(|private| {
                    private.is_ipv4()
                        && private.prefix() <= v4.prefix()
                        && private.contains(std::net::IpAddr::V4(v4.network()))
                })
        }
        IpNetwork::V6(v6) => overlaps(*v6, GLOBAL_UNICAST_IPV6),
    }
}

/// Two prefixes overlap exactly when one contains the other's first address.
fn overlaps(network: Ipv6Network, (address, prefix): (Ipv6Addr, u8)) -> bool {
    let other = Ipv6Network::new(address, prefix).expect("the constant prefixes are valid");
    network.contains(other.network()) || other.contains(network.network())
}

impl Settings {
    /// The user's list, when there is one and it is still valid. A list read
    /// from disk is only as trustworthy as whoever last wrote the file.
    pub fn valid_custom_lan_networks(&self) -> Option<Vec<IpNetwork>> {
        validate_lan_networks(self.custom_lan_networks.clone()?).ok()
    }

    /// The shared networks that reach past the private address space
    /// ([`is_public_lan_network`]), in list order.
    pub fn public_lan_networks(&self) -> Vec<IpNetwork> {
        self.lan_networks()
            .into_iter()
            .filter(is_public_lan_network)
            .collect()
    }

    /// The networks shared while `allow_lan` is on: the user's list when
    /// they customised it, the built-in private ranges otherwise.
    pub fn lan_networks(&self) -> Vec<IpNetwork> {
        self.valid_custom_lan_networks()
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

    #[test]
    fn a_public_range_is_told_apart_from_the_private_ones() {
        for public in [
            "8.0.0.0/8",
            "203.0.113.0/24",
            "2001:db8::/32",
            "2a01:4f8::/32",
        ] {
            assert!(is_public_lan_network(&net(public)), "{public}");
        }
        for private in [
            "10.0.0.0/8",
            "192.168.1.0/24",
            "172.16.0.0/12",
            "169.254.0.0/16",
            "100.64.0.0/10",
            "fd00::/8",
            "fe80::/10",
            "400::/7",
        ] {
            assert!(!is_public_lan_network(&net(private)), "{private}");
        }
    }

    #[test]
    fn only_the_public_ranges_of_a_custom_list_are_reported() {
        let mut settings = Settings {
            custom_lan_networks: Some(vec![net("192.168.0.0/16"), net("8.0.0.0/8")]),
            ..Default::default()
        };

        assert_eq!(settings.public_lan_networks(), vec![net("8.0.0.0/8")]);
        settings.custom_lan_networks = None;
        assert!(settings.public_lan_networks().is_empty());
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

    /// These prefixes embed IPv4 addresses (NAT64, 6to4, Teredo, mapped and
    /// compatible addresses): sharing one, or a range containing one, would
    /// let an IPv6-only network reach any IPv4 host around the tunnel.
    #[test]
    fn rejects_a_network_overlapping_an_ipv4_translation_prefix() {
        for network in [
            "64:ff9b::/96",
            "64:ff9b:1::/48",
            "2002::/16",
            "2001::/32",
            "::ffff:0:0/96",
            "::/96",
            "::/7",
            "2002:c000:0204::/48",
        ] {
            assert_eq!(
                validate_lan_networks(vec![net(network)]),
                Err(LanNetworkError::TranslatesIpv4(net(network))),
                "{network} must be refused"
            );
        }
    }

    /// Global unicast IPv6 is allocated in /16 blocks and wider; anything
    /// wider than a /16 there is a slice of the IPv6 Internet, not a network
    /// someone runs. The overlay ranges outside it (400::/7, 200::/7) stay allowed.
    #[test]
    fn rejects_a_global_unicast_ipv6_network_wider_than_a_slash_16() {
        assert_eq!(
            validate_lan_networks(vec![net("2a00::/12")]),
            Err(LanNetworkError::TooBroad(net("2a00::/12")))
        );
        assert_eq!(
            validate_lan_networks(vec![net("2a01:4f8::/32"), net("200::/7")]),
            Ok(vec![net("2a01:4f8::/32"), net("200::/7")])
        );
    }

    /// Each network becomes firewall rules; an unbounded list is a way to
    /// make applying the blocking policy fail.
    #[test]
    fn rejects_more_than_the_maximum_number_of_networks() {
        let networks: Vec<IpNetwork> = (0..=MAX_LAN_NETWORKS)
            .map(|i| IpNetwork::new(std::net::Ipv4Addr::new(10, 0, i as u8, 0).into(), 24).unwrap())
            .collect();
        assert_eq!(
            validate_lan_networks(networks),
            Err(LanNetworkError::TooMany)
        );
    }

    /// The settings file is only as trustworthy as whoever last wrote it; an
    /// invalid list read from disk falls back to the built-in ranges.
    #[test]
    fn an_invalid_list_read_from_disk_falls_back_to_the_default_ranges() {
        let settings = Settings {
            custom_lan_networks: Some(vec![net("0.0.0.0/0")]),
            ..Default::default()
        };
        assert_eq!(settings.lan_networks(), ALLOWED_LAN_NETS.to_vec());
    }

    /// Emptying the list is a legitimate choice: only the multicast and
    /// broadcast ranges stay reachable.
    #[test]
    fn accepts_an_empty_list() {
        assert_eq!(validate_lan_networks(vec![]), Ok(vec![]));
    }
}
