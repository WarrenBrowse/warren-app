use std::net::IpAddr;

use mullvad_types::settings::{DnsOptions, DnsState};
use talpid_core::firewall::is_local_address;
use talpid_dns::DnsConfig;
use talpid_types::net::{
    BLOCK_ADS, BLOCK_ADULT_CONTENT, BLOCK_GAMBLING, BLOCK_MALWARE, BLOCK_SOCIAL_MEDIA,
    BLOCK_TRACKERS, content_blocking_resolver,
};

/// Return the DNS resolvers to use
pub fn addresses_from_options(options: &DnsOptions) -> DnsConfig {
    let config = match options.state {
        DnsState::Default => {
            let blocked = &options.default_options;
            let mask = [
                (blocked.block_ads, BLOCK_ADS),
                (blocked.block_trackers, BLOCK_TRACKERS),
                (blocked.block_malware, BLOCK_MALWARE),
                (blocked.block_adult_content, BLOCK_ADULT_CONTENT),
                (blocked.block_gambling, BLOCK_GAMBLING),
                (blocked.block_social_media, BLOCK_SOCIAL_MEDIA),
            ]
            .into_iter()
            .filter(|(on, _)| *on)
            .fold(0, |mask, (_, bit)| mask | bit);

            match content_blocking_resolver(mask) {
                Some(resolver) => DnsConfig::from_addresses(&[IpAddr::V4(resolver)], &[]),
                None => DnsConfig::default(),
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
            DnsConfig::from_addresses(&["10.66.255.1".parse().unwrap()], &[],)
        );
    }

    /// Compose default-state options with the given category flags and assert
    /// the resolver is `10.66.255.{octet}`. This locks the wire contract the
    /// exit's kresd listeners decode (`10.66.255.1..63`): the last octet is a
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
            DnsConfig::from_addresses(&[format!("10.66.255.{octet}").parse().unwrap()], &[]),
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
