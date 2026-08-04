//! Reading the tunnel interface the daemon built.
//!
//! The plugin reports what the kernel actually holds rather than what the
//! daemon says it configured. A divergence between the two is what would
//! make NetworkManager reconcile a live tunnel.

use std::net::IpAddr;

use nix::ifaddrs::getifaddrs;
use nix::net::if_::if_nametoindex;

use crate::config::InterfaceAddresses;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("Failed to enumerate the network interfaces")]
    Enumerate(#[source] nix::Error),

    #[error("The tunnel interface is gone")]
    NoSuchInterface(#[source] nix::Error),
}

/// Index of `name`, used as the identity we keep watching.
///
/// Watching the name alone would not do: an index is reused, so a later
/// interface could inherit the name and silently keep the indicator lit for
/// a tunnel that no longer exists.
pub fn index_of(name: &str) -> Result<u32, Error> {
    if_nametoindex(name).map_err(Error::NoSuchInterface)
}

/// Addresses `name` carries right now.
pub fn addresses_of(name: &str) -> Result<InterfaceAddresses, Error> {
    let mut candidates = Vec::new();
    for interface in getifaddrs().map_err(Error::Enumerate)? {
        if interface.interface_name != name {
            continue;
        }
        let Some(address) = interface.address.as_ref() else {
            continue;
        };
        let prefix = interface
            .netmask
            .as_ref()
            .and_then(sockaddr_prefix_len)
            .unwrap_or(0);

        if let Some(v4) = address.as_sockaddr_in() {
            candidates.push((IpAddr::V4(v4.ip()), prefix));
        } else if let Some(v6) = address.as_sockaddr_in6() {
            candidates.push((IpAddr::V6(v6.ip()), prefix));
        }
    }
    Ok(InterfaceAddresses::select(candidates))
}

/// Prefix length of a netmask expressed as a socket address.
fn sockaddr_prefix_len(netmask: &nix::sys::socket::SockaddrStorage) -> Option<u8> {
    if let Some(v4) = netmask.as_sockaddr_in() {
        return Some(ones(&v4.ip().octets()));
    }
    if let Some(v6) = netmask.as_sockaddr_in6() {
        return Some(ones(&v6.ip().octets()));
    }
    None
}

fn ones(mask: &[u8]) -> u8 {
    mask.iter().map(|byte| byte.count_ones() as u8).sum()
}
