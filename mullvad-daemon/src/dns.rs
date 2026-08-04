use std::net::{IpAddr, Ipv4Addr};

use mullvad_types::settings::{DnsOptions, DnsState};
use talpid_core::firewall::is_local_address;
use talpid_dns::DnsConfig;

/// When we want to block certain contents with the help of DNS server side,
/// we compute the resolver IP to use based on these constants. The last
/// byte can be ORed together to combine multiple block lists.
const DNS_BLOCKING_IP_BASE: Ipv4Addr = Ipv4Addr::new(100, 64, 0, 0);
const DNS_AD_BLOCKING_IP_BIT: u8 = 1 << 0; // 0b00000001
const DNS_TRACKER_BLOCKING_IP_BIT: u8 = 1 << 1; // 0b00000010
const DNS_MALWARE_BLOCKING_IP_BIT: u8 = 1 << 2; // 0b00000100
const DNS_ADULT_BLOCKING_IP_BIT: u8 = 1 << 3; // 0b00001000
const DNS_GAMBLING_BLOCKING_IP_BIT: u8 = 1 << 4; // 0b00010000
const DNS_SOCIAL_MEDIA_BLOCKING_IP_BIT: u8 = 1 << 5; // 0b00100000

/// Return the DNS resolvers to use
pub fn addresses_from_options(options: &DnsOptions) -> DnsConfig {
    let config = match options.state {
        DnsState::Default => {
            // Check if we should use a custom blocking DNS resolver.
            // And if so, compute the IP.
            let mut last_byte: u8 = 0;

            if options.default_options.block_ads {
                last_byte |= DNS_AD_BLOCKING_IP_BIT;
            }
            if options.default_options.block_trackers {
                last_byte |= DNS_TRACKER_BLOCKING_IP_BIT;
            }
            if options.default_options.block_malware {
                last_byte |= DNS_MALWARE_BLOCKING_IP_BIT;
            }
            if options.default_options.block_adult_content {
                last_byte |= DNS_ADULT_BLOCKING_IP_BIT;
            }
            if options.default_options.block_gambling {
                last_byte |= DNS_GAMBLING_BLOCKING_IP_BIT;
            }
            if options.default_options.block_social_media {
                last_byte |= DNS_SOCIAL_MEDIA_BLOCKING_IP_BIT;
            }

            if last_byte != 0 {
                let mut dns_ip = DNS_BLOCKING_IP_BASE.octets();
                dns_ip[dns_ip.len() - 1] |= last_byte;
                DnsConfig::from_addresses(&[IpAddr::V4(Ipv4Addr::from(dns_ip))], &[])
            } else {
                DnsConfig::default()
            }
        }
        DnsState::Custom if options.custom_options.addresses.is_empty() => DnsConfig::default(),
        DnsState::Custom => {
            let (non_tunnel_config, tunnel_config): (Vec<_>, Vec<_>) = options
                .custom_options
                .addresses
                .iter()
                .copied()
                // Private IP ranges should not be tunneled
                .partition(|addr| is_local_address(*addr));
            DnsConfig::from_addresses(&tunnel_config, &non_tunnel_config)
        }
    };
    config.allow_external_dns(options.allow_external_dns)
}

#[cfg(test)]
mod test {
    use crate::dns::addresses_from_options;
    use mullvad_types::settings::{CustomDnsOptions, DefaultDnsOptions, DnsOptions, DnsState};
    use talpid_dns::DnsConfig;

    #[test]
    fn test_default_dns() {
        let public_cfg = DnsOptions {
            state: DnsState::Default,
            custom_options: CustomDnsOptions::default(),
            default_options: DefaultDnsOptions::default(),
            allow_external_dns: false,
        };

        assert_eq!(addresses_from_options(&public_cfg), DnsConfig::default());
    }

    #[test]
    fn test_content_blockers() {
        let public_cfg = DnsOptions {
            state: DnsState::Default,
            custom_options: CustomDnsOptions::default(),
            default_options: DefaultDnsOptions {
                block_ads: true,
                ..DefaultDnsOptions::default()
            },
            allow_external_dns: false,
        };

        assert_eq!(
            addresses_from_options(&public_cfg),
            DnsConfig::from_addresses(&["100.64.0.1".parse().unwrap()], &[],)
        );
    }

    /// Compose default-state options with the given category flags and assert
    /// the resolver is `100.64.0.{octet}`. This locks the wire contract the
    /// exit's kresd listeners decode (`100.64.0.1..63`): the last octet is a
    /// bitmask OR of the per-category bits, so changing any value here is a
    /// server-incompatible break.
    fn assert_blocks_to(default_options: DefaultDnsOptions, octet: u8) {
        let cfg = DnsOptions {
            state: DnsState::Default,
            custom_options: CustomDnsOptions::default(),
            default_options,
            allow_external_dns: false,
        };
        assert_eq!(
            addresses_from_options(&cfg),
            DnsConfig::from_addresses(&[format!("100.64.0.{octet}").parse().unwrap()], &[]),
        );
    }

    #[test]
    fn test_each_category_maps_to_its_bit() {
        assert_blocks_to(
            DefaultDnsOptions {
                block_ads: true,
                ..DefaultDnsOptions::default()
            },
            1,
        );
        assert_blocks_to(
            DefaultDnsOptions {
                block_trackers: true,
                ..DefaultDnsOptions::default()
            },
            2,
        );
        assert_blocks_to(
            DefaultDnsOptions {
                block_malware: true,
                ..DefaultDnsOptions::default()
            },
            4,
        );
        assert_blocks_to(
            DefaultDnsOptions {
                block_adult_content: true,
                ..DefaultDnsOptions::default()
            },
            8,
        );
        assert_blocks_to(
            DefaultDnsOptions {
                block_gambling: true,
                ..DefaultDnsOptions::default()
            },
            16,
        );
        assert_blocks_to(
            DefaultDnsOptions {
                block_social_media: true,
                ..DefaultDnsOptions::default()
            },
            32,
        );
    }

    #[test]
    fn test_categories_combine_as_bitmask() {
        // ads(1) | malware(4) = 5: distinct bits OR together, never collide.
        assert_blocks_to(
            DefaultDnsOptions {
                block_ads: true,
                block_malware: true,
                ..DefaultDnsOptions::default()
            },
            5,
        );
        // All six categories = 1|2|4|8|16|32 = 63, the highest listener address.
        assert_blocks_to(
            DefaultDnsOptions {
                block_ads: true,
                block_trackers: true,
                block_malware: true,
                block_adult_content: true,
                block_gambling: true,
                block_social_media: true,
            },
            63,
        );
    }

    // Public IPs should be tunneled, but most private IPs should not be
    #[test]
    fn test_custom_dns() {
        let public_ip = "1.2.3.4".parse().unwrap();
        let private_ip = "172.16.10.1".parse().unwrap();
        let public_cfg = DnsOptions {
            state: DnsState::Custom,
            custom_options: CustomDnsOptions {
                addresses: vec![public_ip, private_ip],
            },
            default_options: DefaultDnsOptions::default(),
            allow_external_dns: false,
        };

        assert_eq!(
            addresses_from_options(&public_cfg),
            DnsConfig::from_addresses(&[public_ip], &[private_ip],)
        );
    }
}
