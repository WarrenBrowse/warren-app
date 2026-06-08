use anyhow::Result;
use clap::Subcommand;
use mullvad_management_interface::MullvadProxyClient;
use mullvad_types::settings::{CustomDnsOptions, DefaultDnsOptions, DnsOptions, DnsState};
use std::net::IpAddr;

#[derive(Subcommand, Debug)]
pub enum Dns {
    /// Display the current DNS settings
    Get,

    /// Set DNS servers to use
    Set {
        #[clap(subcommand)]
        cmd: DnsSet,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum DnsSet {
    /// Use a default DNS server, with or without content
    /// blocking.
    Default {
        /// Block domains known to be used for ads
        #[arg(long)]
        block_ads: bool,

        /// Block domains known to be used for tracking
        #[arg(long)]
        block_trackers: bool,

        /// Block domains known to be used by malware
        #[arg(long)]
        block_malware: bool,

        /// Block domains known to be used for adult content
        #[arg(long)]
        block_adult_content: bool,

        /// Block domains known to be used for gambling
        #[arg(long)]
        block_gambling: bool,

        /// Block domains related to social media
        #[arg(long)]
        block_social_media: bool,
    },

    /// Set a list of custom DNS servers
    Custom {
        /// One or more IP addresses pointing to DNS resolvers
        #[arg(required(true), num_args = 1..)]
        servers: Vec<IpAddr>,
    },

    /// Allow DNS queries to resolvers other than the configured ones (advanced).
    /// When enabled, the firewall no longer blocks port 53 to arbitrary servers, so commands like
    /// `dig @1.1.1.1` work while connected. Queries still egress through the tunnel; the only cost
    /// is that the chosen resolver sees them. This is independent of the default/custom DNS state.
    AllowExternalDns {
        /// Whether to allow external resolvers (true/false)
        #[arg(action = clap::ArgAction::Set)]
        enabled: bool,
    },
}

impl Dns {
    pub async fn handle(self) -> Result<()> {
        match self {
            Dns::Get => Self::get().await,
            Dns::Set {
                cmd:
                    DnsSet::Default {
                        block_ads,
                        block_trackers,
                        block_malware,
                        block_adult_content,
                        block_gambling,
                        block_social_media,
                    },
            } => {
                Self::set_default(
                    block_ads,
                    block_trackers,
                    block_malware,
                    block_adult_content,
                    block_gambling,
                    block_social_media,
                )
                .await
            }
            Dns::Set {
                cmd: DnsSet::Custom { servers },
            } => Self::set_custom(servers).await,
            Dns::Set {
                cmd: DnsSet::AllowExternalDns { enabled },
            } => Self::set_allow_external_dns(enabled).await,
        }
    }

    async fn get() -> Result<()> {
        let mut rpc = MullvadProxyClient::new().await?;
        let options = rpc.get_settings().await?.tunnel_options.dns_options;

        match options.state {
            DnsState::Default => {
                println!("Custom DNS: no");
                println!("Block ads: {}", options.default_options.block_ads);
                println!("Block trackers: {}", options.default_options.block_trackers);
                println!("Block malware: {}", options.default_options.block_malware);
                println!(
                    "Block adult content: {}",
                    options.default_options.block_adult_content
                );
                println!("Block gambling: {}", options.default_options.block_gambling);
                println!(
                    "Block social media: {}",
                    options.default_options.block_social_media
                );
            }
            DnsState::Custom => {
                println!("Custom DNS: yes\nServers:");
                for server in &options.custom_options.addresses {
                    println!("{server}");
                }
            }
        }

        println!("Allow external DNS: {}", options.allow_external_dns);

        Ok(())
    }

    async fn set_default(
        block_ads: bool,
        block_trackers: bool,
        block_malware: bool,
        block_adult_content: bool,
        block_gambling: bool,
        block_social_media: bool,
    ) -> Result<()> {
        let mut rpc = MullvadProxyClient::new().await?;
        let settings = rpc.get_settings().await?;
        rpc.set_dns_options(DnsOptions {
            state: DnsState::Default,
            default_options: DefaultDnsOptions {
                block_ads,
                block_trackers,
                block_malware,
                block_adult_content,
                block_gambling,
                block_social_media,
            },
            ..settings.tunnel_options.dns_options
        })
        .await?;
        println!("Updated DNS settings");
        Ok(())
    }

    async fn set_custom(servers: Vec<IpAddr>) -> Result<()> {
        let mut rpc = MullvadProxyClient::new().await?;
        let settings = rpc.get_settings().await?;
        rpc.set_dns_options(DnsOptions {
            state: DnsState::Custom,
            custom_options: CustomDnsOptions { addresses: servers },
            ..settings.tunnel_options.dns_options
        })
        .await?;
        println!("Updated DNS settings");
        Ok(())
    }

    async fn set_allow_external_dns(enabled: bool) -> Result<()> {
        let mut rpc = MullvadProxyClient::new().await?;
        let settings = rpc.get_settings().await?;
        rpc.set_dns_options(DnsOptions {
            allow_external_dns: enabled,
            ..settings.tunnel_options.dns_options
        })
        .await?;
        println!("Updated DNS settings");
        Ok(())
    }
}
