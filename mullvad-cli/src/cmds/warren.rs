//! Warren fork — Phase F.2 — sous-commandes CLI pour piloter les flags
//! `Settings::warren_mode` et `Settings::warren_local_account` sans
//! avoir à exporter d'env var POC.
//!
//! Le restart du daemon est requis pour appliquer un changement (cf.
//! `warren_mode::resolve` et `warren_account_mode::resolve` côté daemon
//! qui lisent les flags au boot uniquement).

use anyhow::Result;
use clap::Subcommand;
use mullvad_management_interface::MullvadProxyClient;

use super::BooleanOption;

#[derive(Subcommand, Debug)]
pub enum Warren {
    /// Manage the Warren tunnel mode (Iroh QUIC backend) toggle
    #[clap(subcommand)]
    Mode(WarrenMode),

    /// Manage the Warren local account mode (no api.mullvad.net) toggle
    #[clap(subcommand)]
    LocalAccount(WarrenLocalAccount),
}

#[derive(Subcommand, Debug)]
pub enum WarrenMode {
    /// Show the persisted Warren tunnel mode setting
    Get,

    /// Persist the Warren tunnel mode setting (restart daemon to apply)
    Set {
        #[arg(value_parser = BooleanOption::custom_parser("on", "off"))]
        state: BooleanOption,
    },
}

#[derive(Subcommand, Debug)]
pub enum WarrenLocalAccount {
    /// Show the persisted Warren local account setting
    Get,

    /// Persist the Warren local account setting (restart daemon to apply)
    Set {
        #[arg(value_parser = BooleanOption::custom_parser("on", "off"))]
        state: BooleanOption,
    },
}

impl Warren {
    pub async fn handle(self) -> Result<()> {
        match self {
            Warren::Mode(WarrenMode::Get) => Self::mode_get().await,
            Warren::Mode(WarrenMode::Set { state }) => Self::mode_set(*state).await,
            Warren::LocalAccount(WarrenLocalAccount::Get) => Self::local_account_get().await,
            Warren::LocalAccount(WarrenLocalAccount::Set { state }) => {
                Self::local_account_set(*state).await
            }
        }
    }

    async fn mode_get() -> Result<()> {
        let mut rpc = MullvadProxyClient::new().await?;
        let label = BooleanOption::with_labels(rpc.get_settings().await?.warren_mode, "on", "off");
        println!("Warren tunnel mode: {label}");
        Ok(())
    }

    async fn mode_set(state: bool) -> Result<()> {
        let mut rpc = MullvadProxyClient::new().await?;
        rpc.set_warren_mode(state).await?;
        let label = BooleanOption::with_labels(state, "on", "off");
        println!("Warren tunnel mode persisted: {label} (restart `mullvad-daemon` to apply)");
        Ok(())
    }

    async fn local_account_get() -> Result<()> {
        let mut rpc = MullvadProxyClient::new().await?;
        let label =
            BooleanOption::with_labels(rpc.get_settings().await?.warren_local_account, "on", "off");
        println!("Warren local account mode: {label}");
        Ok(())
    }

    async fn local_account_set(state: bool) -> Result<()> {
        let mut rpc = MullvadProxyClient::new().await?;
        rpc.set_warren_local_account(state).await?;
        let label = BooleanOption::with_labels(state, "on", "off");
        println!(
            "Warren local account mode persisted: {label} (restart `mullvad-daemon` to apply)"
        );
        Ok(())
    }
}
