use anyhow::Result;
use clap::Subcommand;
use mullvad_management_interface::MullvadProxyClient;

/// Manage split tunneling. To launch applications outside the tunnel, use the program
/// 'warren-exclude' instead of this command; to launch them inside it while "VPN only for these
/// apps" is on, use 'warren-include'. The processes are the excluded ones, or the included ones
/// in include-only mode.
#[derive(Subcommand, Debug)]
pub enum SplitTunnel {
    /// List all processes that are excluded from the tunnel (included, in include-only mode)
    List,
    /// Add a PID to exclude from the tunnel (to include, in include-only mode)
    Add { pid: i32 },
    /// Stop excluding (including) a PID
    Delete { pid: i32 },
    /// Stop excluding (including) all processes
    Clear,
}

impl SplitTunnel {
    pub async fn handle(self) -> Result<()> {
        match self {
            SplitTunnel::List => {
                let pids = MullvadProxyClient::new()
                    .await?
                    .get_split_tunnel_processes()
                    .await?;

                println!("Split PIDs:");
                for pid in &pids {
                    println!("{pid}");
                }

                Ok(())
            }
            SplitTunnel::Add { pid } => {
                MullvadProxyClient::new()
                    .await?
                    .add_split_tunnel_process(pid)
                    .await?;
                println!("Splitting process");
                Ok(())
            }
            SplitTunnel::Delete { pid } => {
                MullvadProxyClient::new()
                    .await?
                    .remove_split_tunnel_process(pid)
                    .await?;
                println!("Stopped splitting process");
                Ok(())
            }
            SplitTunnel::Clear => {
                MullvadProxyClient::new()
                    .await?
                    .clear_split_tunnel_processes()
                    .await?;
                println!("Stopped splitting all processes");
                Ok(())
            }
        }
    }
}
