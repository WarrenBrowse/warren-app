use anyhow::{Context, Result};
use mullvad_management_interface::MullvadProxyClient;

pub async fn print() -> Result<()> {
    println!("{:22}: {}", "Current version", mullvad_version::VERSION);

    let mut rpc = MullvadProxyClient::new()
        .await
        .context("Failed to connect to the Warren daemon")?;

    let daemon_version = rpc
        .get_current_version()
        .await
        .context("Failed to get current Warren daemon version")?;

    if daemon_version != mullvad_version::VERSION {
        println!("{:22}: {}", "Warren daemon version", daemon_version);
    };

    let version_info = rpc
        .get_version_info()
        .await
        .context("Failed to get version info")?;
    println!(
        "{:22}: {}",
        "Is supported", version_info.current_version_supported
    );

    if let Some(suggested_upgrade) = version_info.suggested_upgrade {
        println!("{:22}: {}", "Suggested upgrade", suggested_upgrade.version);
    } else {
        println!("{:22}: none", "Suggested upgrade");
    }

    Ok(())
}
