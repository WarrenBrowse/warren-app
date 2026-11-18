use ipnetwork::Ipv4Network;
use std::net::Ipv4Addr;

/// The exit's content-blocking resolvers. The last octet is the set of blocked categories, one
/// bit each, so one address serves every combination; the exit decodes it the same way.
///
/// The block sits inside the exit's own pool ([`super::ALLOWED_IN_TUNNEL_LAN_NETS`]): it is
/// routed into the tunnel and admitted there like the gateway, and no address in it is ever a
/// client's. Never inside `100.64.0.0/10`: Tailscale and other overlays claim that range, and
/// their routes and the firewall passes made for them would capture the queries.
pub const CONTENT_BLOCKING_RESOLVERS: Ipv4Network =
    Ipv4Network::new_checked(Ipv4Addr::new(10, 66, 255, 0), 26).unwrap();

pub const BLOCK_ADS: u8 = 1 << 0;
pub const BLOCK_TRACKERS: u8 = 1 << 1;
pub const BLOCK_MALWARE: u8 = 1 << 2;
pub const BLOCK_ADULT_CONTENT: u8 = 1 << 3;
pub const BLOCK_GAMBLING: u8 = 1 << 4;
pub const BLOCK_SOCIAL_MEDIA: u8 = 1 << 5;

/// The resolver that blocks the categories in `mask`, an OR of the `BLOCK_*` bits. `None` when
/// the mask blocks nothing or names a bit no resolver serves.
pub fn content_blocking_resolver(mask: u8) -> Option<Ipv4Addr> {
    let host_bits = !u32::from(CONTENT_BLOCKING_RESOLVERS.mask());
    if mask == 0 || u32::from(mask) & !host_bits != 0 {
        return None;
    }
    Some(Ipv4Addr::from(
        u32::from(CONTENT_BLOCKING_RESOLVERS.network()) | u32::from(mask),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocking_nothing_names_no_resolver() {
        assert_eq!(content_blocking_resolver(0), None);
    }

    #[test]
    fn a_bit_beyond_the_known_categories_names_no_resolver() {
        assert_eq!(content_blocking_resolver(1 << 6), None);
        assert_eq!(content_blocking_resolver(u8::MAX), None);
    }

    #[test]
    fn the_resolver_is_the_exits_address_for_that_set_of_categories() {
        for (mask, resolver) in [
            (BLOCK_ADS, "10.66.255.1"),
            (BLOCK_ADS | BLOCK_TRACKERS, "10.66.255.3"),
            (BLOCK_MALWARE, "10.66.255.4"),
            (BLOCK_ADULT_CONTENT, "10.66.255.8"),
            (BLOCK_GAMBLING, "10.66.255.16"),
            (BLOCK_SOCIAL_MEDIA, "10.66.255.32"),
            (63, "10.66.255.63"),
        ] {
            assert_eq!(
                content_blocking_resolver(mask),
                Some(resolver.parse().unwrap()),
                "mask {mask}"
            );
        }
    }

    /// Inside the exit's pool, which every client routes into the tunnel and the firewall admits
    /// there, and which no LAN share or overlay network opens.
    #[test]
    fn the_resolvers_live_inside_the_exits_own_pool() {
        let pool = crate::net::ALLOWED_IN_TUNNEL_LAN_NETS[0];
        assert!(pool.contains(CONTENT_BLOCKING_RESOLVERS.network().into()));
        assert!(pool.contains(CONTENT_BLOCKING_RESOLVERS.broadcast().into()));
    }

    #[test]
    fn every_combination_stays_inside_the_resolver_block() {
        for mask in 1..=63u8 {
            let resolver = content_blocking_resolver(mask).expect("a known combination");
            assert!(CONTENT_BLOCKING_RESOLVERS.contains(resolver), "{resolver}");
            assert_eq!(resolver.octets()[3], mask);
        }
    }
}
