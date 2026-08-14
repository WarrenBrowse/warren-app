use anyhow::{Context, Result};
use mullvad_management_interface::MullvadProxyClient;

/// What `warren version` can state from the binary alone, before it tries to
/// reach a daemon.
///
/// The daemon is the one thing that is not available exactly when this matters:
/// on a headless install that will not start. The API host is compiled in, and
/// a build pointed at the wrong environment is indistinguishable from a working
/// one until something fails to connect, so the CLI says which network it
/// targets whether or not anything is listening.
fn local_version_lines() -> [(&'static str, &'static str); 3] {
    [
        ("Current version", mullvad_version::VERSION),
        ("Product environment", warren_product_env::ENV_NAME),
        ("API host", warren_product_env::API_HOST),
    ]
}

pub async fn print() -> Result<()> {
    for (label, value) in local_version_lines() {
        println!("{label:22}: {value}");
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    /// The whole point of these lines is that they survive a dead daemon, so
    /// they are asserted independently of one.
    #[test]
    fn version_names_the_network_the_binary_targets() {
        let lines = local_version_lines();
        let value_of = |label: &str| {
            lines
                .iter()
                .find(|(name, _)| *name == label)
                .map(|(_, value)| *value)
        };

        assert_eq!(value_of("Current version"), Some(mullvad_version::VERSION));
        assert_eq!(
            value_of("Product environment"),
            Some(warren_product_env::ENV_NAME)
        );
        assert_eq!(value_of("API host"), Some(warren_product_env::API_HOST));
    }
}
