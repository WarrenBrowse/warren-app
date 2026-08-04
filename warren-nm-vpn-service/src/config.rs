//! Turning what NetworkManager hands us into what we hand back.
//!
//! Kept free of D-Bus and of the kernel so the whole decision layer is
//! testable on any host: the plugin's only job is to describe a tunnel the
//! daemon has already built, and describing it wrong is the one way this
//! component can hurt the datapath.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// Key carrying the tunnel interface name in NetworkManager's `vpn.data`.
pub const DATA_TUNDEV: &str = "tundev";
/// Key carrying the address of the host the tunnel talks to.
pub const DATA_GATEWAY: &str = "gateway";

/// What the daemon asked us to publish, read from `vpn.data`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectRequest {
    /// Interface the daemon already created and configured.
    pub tundev: String,
    /// The peer the tunnel is established with. NetworkManager refuses a
    /// VPN config that carries no external gateway, so this is required.
    pub gateway: IpAddr,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum RequestError {
    #[error("The connection carries no tunnel interface name")]
    MissingTundev,

    #[error("The connection carries no gateway address")]
    MissingGateway,

    #[error("The gateway address is not an IP address")]
    InvalidGateway,
}

impl ConnectRequest {
    pub fn from_vpn_data(data: &HashMap<String, String>) -> Result<Self, RequestError> {
        let tundev = data
            .get(DATA_TUNDEV)
            .filter(|name| !name.is_empty())
            .ok_or(RequestError::MissingTundev)?
            .clone();
        let gateway = data
            .get(DATA_GATEWAY)
            .filter(|gateway| !gateway.is_empty())
            .ok_or(RequestError::MissingGateway)?
            .parse()
            .map_err(|_| RequestError::InvalidGateway)?;
        Ok(ConnectRequest { tundev, gateway })
    }
}

/// The addresses actually carried by the tunnel interface.
///
/// Read from the live interface rather than passed in by the daemon: what
/// we publish has to match what the kernel holds, or NetworkManager would
/// reconcile the difference onto a live tunnel.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InterfaceAddresses {
    pub v4: Option<(Ipv4Addr, u8)>,
    pub v6: Option<(Ipv6Addr, u8)>,
}

impl InterfaceAddresses {
    /// Pick the addresses worth publishing out of everything the interface
    /// carries. IPv6 link-local is skipped: the kernel puts one on every
    /// interface, and publishing it would describe the tunnel as reaching
    /// nothing but its own link.
    pub fn select(candidates: impl IntoIterator<Item = (IpAddr, u8)>) -> Self {
        let mut selected = InterfaceAddresses::default();
        for (address, prefix) in candidates {
            match address {
                IpAddr::V4(v4) if selected.v4.is_none() && !v4.is_loopback() => {
                    selected.v4 = Some((v4, prefix));
                }
                IpAddr::V6(v6)
                    if selected.v6.is_none()
                        && !v6.is_loopback()
                        && !is_unicast_link_local(&v6) =>
                {
                    selected.v6 = Some((v6, prefix));
                }
                _ => {}
            }
        }
        selected
    }

    pub fn is_empty(&self) -> bool {
        self.v4.is_none() && self.v6.is_none()
    }
}

/// `Ipv6Addr::is_unicast_link_local` is still unstable, so spell fe80::/10 out.
fn is_unicast_link_local(address: &Ipv6Addr) -> bool {
    address.segments()[0] & 0xffc0 == 0xfe80
}

/// Everything the plugin is about to tell NetworkManager.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigPlan {
    pub tundev: String,
    pub gateway: IpAddr,
    pub addresses: InterfaceAddresses,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum PlanError {
    #[error("The tunnel interface carries no address to publish")]
    NoAddresses,
}

impl ConfigPlan {
    pub fn build(
        request: ConnectRequest,
        addresses: InterfaceAddresses,
    ) -> Result<Self, PlanError> {
        if addresses.is_empty() {
            return Err(PlanError::NoAddresses);
        }
        Ok(ConfigPlan {
            tundev: request.tundev,
            gateway: request.gateway,
            addresses,
        })
    }

    pub fn has_ip4(&self) -> bool {
        self.addresses.v4.is_some()
    }

    pub fn has_ip6(&self) -> bool {
        self.addresses.v6.is_some()
    }

    /// The `Config` dictionary NetworkManager is handed.
    pub fn config_entries(&self) -> Vec<(&'static str, ConfigValue)> {
        vec![
            ("tundev", ConfigValue::Text(self.tundev.clone())),
            ("gateway", ConfigValue::address(self.gateway)),
            ("has-ip4", ConfigValue::Flag(self.has_ip4())),
            ("has-ip6", ConfigValue::Flag(self.has_ip6())),
            // The daemon owns the tunnel's lifetime. Letting NetworkManager
            // keep the connection across its own restart would outlive it.
            ("can-persist", ConfigValue::Flag(false)),
        ]
    }

    /// The `Ip4Config` / `Ip6Config` dictionary for a family the tunnel
    /// carries, or `None` for one it does not.
    pub fn ip_entries(&self, family: Family) -> Option<Vec<(&'static str, ConfigValue)>> {
        let carried = match family {
            Family::V4 => self.has_ip4(),
            Family::V6 => self.has_ip6(),
        };
        if !carried {
            return None;
        }
        // The address is what the interface actually carries, read from the
        // kernel, never a value of our own. NetworkManager reconciles the
        // config it is given onto the interface, so anything else here would
        // be applied to a live tunnel.
        //
        // NetworkManager adopts the address, which means it also removes it
        // when the connection is deactivated (measured on NM 1.46). That is
        // safe because the removal targets the interface index recorded at
        // connect time: the daemon withdraws while that tunnel is the one
        // still in place, and a later tunnel is a new interface with a new
        // index. Announcing nothing is not an option, NM 1.46 rejects a
        // config with no address.
        //
        // `never-default` and `preserve-routes` cover routing: the engine
        // installs the split default and the per-exit host routes, and
        // NetworkManager must neither add to that nor replace it.
        let mut entries = vec![
            ("never-default", ConfigValue::Flag(true)),
            ("preserve-routes", ConfigValue::Flag(true)),
        ];
        match family {
            Family::V4 => {
                let (address, prefix) = self.addresses.v4?;
                entries.push(("address", ConfigValue::Number(nm_ipv4(address))));
                entries.push(("prefix", ConfigValue::Number(u32::from(prefix))));
            }
            Family::V6 => {
                let (address, prefix) = self.addresses.v6?;
                entries.push(("address", ConfigValue::Bytes(address.octets().to_vec())));
                entries.push(("prefix", ConfigValue::Number(u32::from(prefix))));
            }
        }
        Some(entries)
    }
}

/// An IP family, for picking which dictionary to build.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    V4,
    V6,
}

/// A value in one of the dictionaries handed to NetworkManager, in the
/// shape that side reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigValue {
    Text(String),
    Flag(bool),
    /// Read by NetworkManager as a native `u32`, which is how it encodes an
    /// IPv4 address as well as a plain number.
    Number(u32),
    /// Read by NetworkManager as `ay`, which is how an IPv6 address travels.
    Bytes(Vec<u8>),
}

impl ConfigValue {
    fn address(address: IpAddr) -> Self {
        match address {
            IpAddr::V4(v4) => ConfigValue::Number(nm_ipv4(v4)),
            IpAddr::V6(v6) => ConfigValue::Bytes(v6.octets().to_vec()),
        }
    }
}

/// The tunnel the plugin described, kept so that a tunnel which ends takes
/// the desktop indicator with it.
///
/// NetworkManager keeps a VPN connection activated after its interface has
/// gone (measured on NM 1.46), so a daemon that dies would otherwise leave
/// the desktop claiming a VPN that carries nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TunnelIdentity {
    pub tundev: String,
    pub ifindex: u32,
}

impl TunnelIdentity {
    /// Whether what we described is no longer what the machine has.
    ///
    /// Compared by index rather than by name: indices are reused, so a
    /// later interface inheriting the name would otherwise keep the
    /// indicator lit for a tunnel that already ended.
    pub fn is_stale(&self, current_index: Option<u32>) -> bool {
        current_index != Some(self.ifindex)
    }
}

/// Encode an IPv4 address the way NetworkManager reads it back.
///
/// NM reads the value as a native `u32` and writes those bytes straight out
/// as an address, so the native byte order has to already hold the network
/// order bytes. Getting this backwards publishes a mirrored address.
pub fn nm_ipv4(address: Ipv4Addr) -> u32 {
    u32::from_ne_bytes(address.octets())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vpn_data(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
            .collect()
    }

    #[test]
    fn reads_the_interface_and_gateway_the_daemon_published() {
        let data = vpn_data(&[("tundev", "warren0"), ("gateway", "203.0.113.7")]);

        let request = ConnectRequest::from_vpn_data(&data).unwrap();

        assert_eq!(request.tundev, "warren0");
        assert_eq!(request.gateway, "203.0.113.7".parse::<IpAddr>().unwrap());
    }

    #[test]
    fn rejects_a_connection_without_an_interface() {
        let data = vpn_data(&[("gateway", "203.0.113.7")]);

        assert_eq!(
            ConnectRequest::from_vpn_data(&data),
            Err(RequestError::MissingTundev)
        );
    }

    #[test]
    fn rejects_an_empty_interface_name() {
        let data = vpn_data(&[("tundev", ""), ("gateway", "203.0.113.7")]);

        assert_eq!(
            ConnectRequest::from_vpn_data(&data),
            Err(RequestError::MissingTundev)
        );
    }

    #[test]
    fn rejects_a_connection_without_a_gateway() {
        let data = vpn_data(&[("tundev", "warren0")]);

        assert_eq!(
            ConnectRequest::from_vpn_data(&data),
            Err(RequestError::MissingGateway)
        );
    }

    #[test]
    fn rejects_a_gateway_that_is_not_an_address() {
        let data = vpn_data(&[("tundev", "warren0"), ("gateway", "exit.example.com")]);

        assert_eq!(
            ConnectRequest::from_vpn_data(&data),
            Err(RequestError::InvalidGateway)
        );
    }

    #[test]
    fn accepts_an_ipv6_gateway() {
        let data = vpn_data(&[("tundev", "warren0"), ("gateway", "2001:db8::1")]);

        let request = ConnectRequest::from_vpn_data(&data).unwrap();

        assert_eq!(request.gateway, "2001:db8::1".parse::<IpAddr>().unwrap());
    }

    #[test]
    fn selects_the_first_routable_address_of_each_family() {
        let selected = InterfaceAddresses::select([
            ("10.66.0.2".parse().unwrap(), 24),
            ("10.66.0.3".parse().unwrap(), 24),
            ("fdcc:f:1::2".parse().unwrap(), 64),
        ]);

        assert_eq!(selected.v4, Some(("10.66.0.2".parse().unwrap(), 24)));
        assert_eq!(selected.v6, Some(("fdcc:f:1::2".parse().unwrap(), 64)));
    }

    #[test]
    fn skips_the_link_local_address_the_kernel_adds() {
        let selected = InterfaceAddresses::select([
            ("fe80::6869:35bf:7be4:770b".parse().unwrap(), 64),
            ("fdcc:f:1::2".parse().unwrap(), 64),
        ]);

        assert_eq!(selected.v6, Some(("fdcc:f:1::2".parse().unwrap(), 64)));
    }

    #[test]
    fn publishes_nothing_when_only_a_link_local_address_is_present() {
        let selected =
            InterfaceAddresses::select([("fe80::6869:35bf:7be4:770b".parse().unwrap(), 64)]);

        assert!(selected.is_empty());
    }

    #[test]
    fn refuses_to_describe_an_interface_that_carries_no_address() {
        let request = ConnectRequest {
            tundev: "warren0".to_owned(),
            gateway: "203.0.113.7".parse().unwrap(),
        };

        assert_eq!(
            ConfigPlan::build(request, InterfaceAddresses::default()),
            Err(PlanError::NoAddresses)
        );
    }

    #[test]
    fn announces_only_the_families_the_tunnel_actually_carries() {
        let request = ConnectRequest {
            tundev: "warren0".to_owned(),
            gateway: "203.0.113.7".parse().unwrap(),
        };
        let addresses = InterfaceAddresses::select([("10.66.0.2".parse().unwrap(), 24)]);

        let plan = ConfigPlan::build(request, addresses).unwrap();

        assert!(plan.has_ip4());
        assert!(!plan.has_ip6());
    }

    #[test]
    fn announces_both_families_on_a_dual_stack_tunnel() {
        let request = ConnectRequest {
            tundev: "warren0".to_owned(),
            gateway: "203.0.113.7".parse().unwrap(),
        };
        let addresses = InterfaceAddresses::select([
            ("10.66.0.2".parse().unwrap(), 24),
            ("fdcc:f:1::2".parse().unwrap(), 64),
        ]);

        let plan = ConfigPlan::build(request, addresses).unwrap();

        assert!(plan.has_ip4());
        assert!(plan.has_ip6());
    }

    fn dual_stack_plan() -> ConfigPlan {
        let request = ConnectRequest {
            tundev: "warren0".to_owned(),
            gateway: "203.0.113.7".parse().unwrap(),
        };
        let addresses = InterfaceAddresses::select([
            ("10.66.0.2".parse().unwrap(), 24),
            ("fdcc:f:1::2".parse().unwrap(), 64),
        ]);
        ConfigPlan::build(request, addresses).unwrap()
    }

    fn key_of(entries: &[(&'static str, ConfigValue)], key: &str) -> Option<ConfigValue> {
        entries
            .iter()
            .find(|(name, _)| *name == key)
            .map(|(_, value)| value.clone())
    }

    /// NetworkManager reconciles whatever config it is handed onto the live
    /// interface, so the published address has to be the one the interface
    /// already carries. A value of our own would be applied to the tunnel.
    #[test]
    fn publishes_exactly_the_addresses_the_interface_carries() {
        let plan = dual_stack_plan();

        let v4 = plan.ip_entries(Family::V4).unwrap();
        assert_eq!(
            key_of(&v4, "address"),
            Some(ConfigValue::Number(nm_ipv4("10.66.0.2".parse().unwrap())))
        );
        assert_eq!(key_of(&v4, "prefix"), Some(ConfigValue::Number(24)));

        let expected_v6 = "fdcc:f:1::2".parse::<std::net::Ipv6Addr>().unwrap();
        let v6 = plan.ip_entries(Family::V6).unwrap();
        assert_eq!(
            key_of(&v6, "address"),
            Some(ConfigValue::Bytes(expected_v6.octets().to_vec()))
        );
        assert_eq!(key_of(&v6, "prefix"), Some(ConfigValue::Number(64)));
    }

    #[test]
    fn tells_networkmanager_to_leave_routing_alone() {
        let entries = dual_stack_plan().ip_entries(Family::V4).unwrap();

        assert_eq!(
            key_of(&entries, "never-default"),
            Some(ConfigValue::Flag(true))
        );
        assert_eq!(
            key_of(&entries, "preserve-routes"),
            Some(ConfigValue::Flag(true))
        );
    }

    #[test]
    fn describes_no_config_for_a_family_the_tunnel_does_not_carry() {
        let request = ConnectRequest {
            tundev: "warren0".to_owned(),
            gateway: "203.0.113.7".parse().unwrap(),
        };
        let addresses = InterfaceAddresses::select([("10.66.0.2".parse().unwrap(), 24)]);
        let plan = ConfigPlan::build(request, addresses).unwrap();

        assert!(plan.ip_entries(Family::V4).is_some());
        assert_eq!(plan.ip_entries(Family::V6), None);
    }

    #[test]
    fn names_the_interface_and_the_peer_in_the_config() {
        let entries = dual_stack_plan().config_entries();

        assert_eq!(
            key_of(&entries, "tundev"),
            Some(ConfigValue::Text("warren0".to_owned()))
        );
        assert_eq!(
            key_of(&entries, "gateway"),
            Some(ConfigValue::Number(nm_ipv4("203.0.113.7".parse().unwrap())))
        );
        assert_eq!(
            key_of(&entries, "can-persist"),
            Some(ConfigValue::Flag(false))
        );
    }

    #[test]
    fn hands_an_ipv6_peer_over_as_raw_bytes() {
        let request = ConnectRequest {
            tundev: "warren0".to_owned(),
            gateway: "2001:db8::1".parse().unwrap(),
        };
        let addresses = InterfaceAddresses::select([("10.66.0.2".parse().unwrap(), 24)]);
        let plan = ConfigPlan::build(request, addresses).unwrap();

        let expected = "2001:db8::1".parse::<std::net::Ipv6Addr>().unwrap();
        assert_eq!(
            key_of(&plan.config_entries(), "gateway"),
            Some(ConfigValue::Bytes(expected.octets().to_vec()))
        );
    }

    fn watched(ifindex: u32) -> TunnelIdentity {
        TunnelIdentity {
            tundev: "warren0".to_owned(),
            ifindex,
        }
    }

    #[test]
    fn keeps_watching_while_the_same_interface_is_in_place() {
        assert!(!watched(7).is_stale(Some(7)));
    }

    #[test]
    fn goes_stale_when_the_interface_disappears() {
        assert!(watched(7).is_stale(None));
    }

    #[test]
    fn goes_stale_when_another_interface_inherits_the_name() {
        assert!(watched(7).is_stale(Some(9)));
    }

    #[test]
    fn encodes_ipv4_so_the_native_bytes_are_the_network_order_ones() {
        let encoded = nm_ipv4("10.66.0.2".parse().unwrap());

        assert_eq!(encoded.to_ne_bytes(), [10, 66, 0, 2]);
    }
}
