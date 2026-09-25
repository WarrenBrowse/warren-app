use anyhow::{Result, bail};
use clap::Subcommand;
use ipnetwork::IpNetwork;
use mullvad_management_interface::MullvadProxyClient;

use super::BooleanOption;

#[derive(Subcommand, Debug)]
pub enum Lan {
    /// Display the current local network sharing setting and the networks it shares
    Get,

    /// Change allow LAN setting
    Set {
        #[arg(value_parser = BooleanOption::custom_parser("allow", "block"))]
        policy: BooleanOption,
    },

    /// Manage the networks reachable outside the tunnel while local network sharing is on
    #[clap(subcommand)]
    Networks(Networks),
}

#[derive(Subcommand, Debug)]
pub enum Networks {
    /// Share one more network, in CIDR notation (e.g. 400::/7)
    Add { network: IpNetwork },

    /// Stop sharing a network
    Remove { network: IpNetwork },

    /// Share the built-in private ranges again
    Reset,
}

impl Lan {
    pub async fn handle(self) -> Result<()> {
        match self {
            Lan::Get => Self::get().await,
            Lan::Set { policy } => Self::set(policy).await,
            Lan::Networks(networks) => networks.handle().await,
        }
    }

    async fn set(policy: BooleanOption) -> Result<()> {
        let mut rpc = MullvadProxyClient::new().await?;
        rpc.set_allow_lan(*policy).await?;
        println!("Changed local network sharing setting");
        Ok(())
    }

    async fn get() -> Result<()> {
        let mut rpc = MullvadProxyClient::new().await?;
        let settings = rpc.get_settings().await?;
        let allow_lan = BooleanOption::with_labels(settings.allow_lan, "allow", "block");
        println!("Local network sharing setting: {allow_lan}");
        let origin = if settings.custom_lan_networks.is_some() {
            "custom"
        } else {
            "default"
        };
        println!("Shared networks ({origin}):");
        for network in settings.lan_networks() {
            println!("\t{network}");
        }
        Ok(())
    }
}

impl Networks {
    async fn handle(self) -> Result<()> {
        let mut rpc = MullvadProxyClient::new().await?;
        let mut networks = rpc.get_settings().await?.lan_networks();
        match self {
            Networks::Add { network } => networks.push(network),
            Networks::Remove { network } => {
                let before = networks.len();
                networks.retain(|shared| *shared != network);
                if networks.len() == before {
                    bail!("{network} is not a shared network");
                }
            }
            Networks::Reset => {
                rpc.set_lan_networks(None).await?;
                println!("Local network sharing uses the built-in private ranges again");
                return Ok(());
            }
        }
        rpc.set_lan_networks(Some(networks)).await?;
        println!("Changed the networks shared by local network sharing");
        Ok(())
    }
}
