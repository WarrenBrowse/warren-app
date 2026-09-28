use ipnetwork::IpNetwork;
use mullvad_daemon::settings::{self, SettingsPersister};
use talpid_core::firewall::{self, Firewall, FirewallPolicy};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Failed to initialize firewall")]
    Firewall(#[from] firewall::Error),

    #[error("Failed to get settings path")]
    Path(#[from] mullvad_paths::Error),

    #[error("Failed to get settings")]
    Settings(#[from] settings::Error),
}

pub async fn initialize_firewall() -> Result<(), Error> {
    let mut firewall = Firewall::new(mullvad_types::TUNNEL_FWMARK, None, None)?;
    let (allow_lan, lan_networks) = get_lan_sharing().await.unwrap_or_else(|err| {
        log::info!(
            "Not allowing LAN traffic due to failing to read settings: {}",
            err
        );
        (false, vec![])
    });
    let policy = FirewallPolicy::Blocked {
        allow_lan,
        lan_networks,
        allowed_endpoint: None,
    };
    log::info!("Applying firewall policy {policy}");
    firewall.apply_policy(policy)?;
    // This process exits right away and the block has to hold until the daemon replaces it.
    firewall.keep_policy_on_drop();
    Ok(())
}

async fn get_lan_sharing() -> Result<(bool, Vec<IpNetwork>), Error> {
    let path = mullvad_paths::settings_dir()?;
    // NOTE: This may fail if the daemon has not been restarted after an upgrade.
    //       This will cause `allow_lan` to be disabled during early boot. This
    //       is probably acceptable.
    let settings = SettingsPersister::read_only(&path).await;
    Ok((settings.allow_lan, settings.lan_networks()))
}

/// Against the kernel's nftables: run as root, or in a network namespace with `CAP_NET_ADMIN`
/// (`docker run --cap-add NET_ADMIN`).
#[cfg(test)]
mod tests {
    use super::*;

    fn table_present() -> bool {
        let tables = std::process::Command::new("nft")
            .args(["list", "tables"])
            .output()
            .expect("nft is installed");
        String::from_utf8_lossy(&tables.stdout).contains(&format!(
            "table inet {}",
            warren_product_env::CURRENT.firewall_id()
        ))
    }

    #[tokio::test]
    #[ignore = "needs CAP_NET_ADMIN"]
    async fn the_early_boot_block_outlives_the_process_that_applies_it() {
        initialize_firewall().await.unwrap();

        let present = table_present();
        Firewall::new(mullvad_types::TUNNEL_FWMARK, None, None)
            .unwrap()
            .reset_policy()
            .unwrap();
        assert!(
            present,
            "the early-boot block was lifted when its firewall was dropped"
        );
    }
}
