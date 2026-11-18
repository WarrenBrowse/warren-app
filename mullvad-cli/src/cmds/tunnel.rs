use anyhow::Result;
use clap::Subcommand;
use mullvad_management_interface::MullvadProxyClient;
use mullvad_types::wireguard::QuantumResistantState;

use super::BooleanOption;
use crate::print_option;

#[derive(Subcommand, Debug)]
pub enum Tunnel {
    /// Show current tunnel options
    Get,

    /// Set tunnel options
    #[clap(subcommand)]
    Set(TunnelOptions),
}

#[derive(Subcommand, Debug, Clone)]
pub enum TunnelOptions {
    /// Configure quantum-resistant key exchange
    QuantumResistant { state: QuantumResistantState },

    /// Configure whether to enable DAITA
    Daita { state: BooleanOption },

    /// Configure whether to enable DAITA direct only
    DaitaDirectOnly { state: BooleanOption },

    /// Enable or disable IPv6 in the tunnel
    #[clap(arg_required_else_help = true)]
    Ipv6 { state: BooleanOption },
}

impl Tunnel {
    pub async fn handle(self) -> Result<()> {
        match self {
            Tunnel::Get => Self::get().await,
            Tunnel::Set(options) => Self::set(options).await,
        }
    }

    async fn get() -> Result<()> {
        let mut rpc = MullvadProxyClient::new().await?;
        let tunnel_options = rpc.get_settings().await?.tunnel_options;

        println!("WireGuard options");

        print_option!(
            "Quantum resistance",
            tunnel_options.wireguard.quantum_resistant,
        );

        print_option!("DAITA", tunnel_options.wireguard.daita.enabled);

        println!("Generic options");

        print_option!(
            "IPv6",
            if tunnel_options.generic.enable_ipv6 {
                "on"
            } else {
                "off"
            }
        );

        Ok(())
    }

    async fn set(options: TunnelOptions) -> Result<()> {
        let mut rpc = MullvadProxyClient::new().await?;

        match options {
            TunnelOptions::QuantumResistant { state } => {
                rpc.set_quantum_resistant_tunnel(state).await?;
                println!("Quantum resistant setting has been updated");
            }
            TunnelOptions::Daita { state } => {
                rpc.set_enable_daita(*state).await?;
                println!("DAITA setting has been updated");
            }
            TunnelOptions::DaitaDirectOnly { state } => {
                rpc.set_daita_direct_only(*state).await?;
                println!("Direct only setting has been updated");
            }
            TunnelOptions::Ipv6 { state } => {
                rpc.set_enable_ipv6(*state).await?;
                println!("IPv6: {state}");
            }
        }

        Ok(())
    }
}
