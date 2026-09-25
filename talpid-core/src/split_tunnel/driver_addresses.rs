//! The addresses the Windows split tunnel driver is given.
//!
//! The driver knows two addresses: its "tunnel" address, which the apps it
//! splits must not use, and its "internet" address, which it rebinds them to.
//! Exclusion hands it the VPN address as "tunnel" and the physical one as
//! "internet", so excluded apps leave on the physical network. Include-only
//! hands it the same pair swapped, with the included apps as the split ones:
//! they are rebound to the VPN address and blocked on the physical one, while
//! the driver leaves every other app alone.

use std::net::{Ipv4Addr, Ipv6Addr};

use talpid_types::split_tunnel::SplitTunnelMode;

/// One address per family.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct AddressPair {
    pub ipv4: Option<Ipv4Addr>,
    pub ipv6: Option<Ipv6Addr>,
}

/// The pair in the driver's terms.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DriverAddresses {
    /// The address the split apps must not use.
    pub tunnel: AddressPair,
    /// The address the split apps are moved to.
    pub internet: AddressPair,
}

/// What the driver is given under `mode` for the VPN interface's and the
/// physical default interface's addresses.
pub fn driver_addresses(
    mode: SplitTunnelMode,
    vpn: AddressPair,
    physical: AddressPair,
) -> DriverAddresses {
    match mode {
        SplitTunnelMode::Exclude => DriverAddresses {
            tunnel: vpn,
            internet: physical,
        },
        SplitTunnelMode::IncludeOnly => DriverAddresses {
            tunnel: physical,
            internet: vpn,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VPN: AddressPair = AddressPair {
        ipv4: Some(Ipv4Addr::new(10, 66, 0, 2)),
        ipv6: None,
    };
    const PHYSICAL: AddressPair = AddressPair {
        ipv4: Some(Ipv4Addr::new(192, 168, 1, 20)),
        ipv6: Some(Ipv6Addr::new(0x2a01, 0xe0a, 0, 0, 0, 0, 0, 0x20)),
    };

    #[test]
    fn exclusion_moves_the_split_apps_to_the_physical_address() {
        let addresses = driver_addresses(SplitTunnelMode::Exclude, VPN, PHYSICAL);

        assert_eq!(addresses.tunnel, VPN);
        assert_eq!(addresses.internet, PHYSICAL);
    }

    #[test]
    fn include_only_moves_the_split_apps_to_the_vpn_address() {
        let addresses = driver_addresses(SplitTunnelMode::IncludeOnly, VPN, PHYSICAL);

        assert_eq!(addresses.tunnel, PHYSICAL);
        assert_eq!(addresses.internet, VPN);
    }
}
