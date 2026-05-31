use crate::BIN_NAME;
use anyhow::Result;
use clap::Subcommand;
use itertools::Itertools;
use mullvad_management_interface::MullvadProxyClient;
use mullvad_types::{account::AccountNumber, device::DeviceState};
use std::io::{self, Write};

const NOT_LOGGED_IN_MESSAGE: &str = "Not logged in on any account";
const REVOKED_MESSAGE: &str = "The current device has been revoked";

#[derive(Subcommand, Debug)]
pub enum Account {
    /// Create and log in on a new account
    Create,

    /// Log in on an account
    Login {
        /// The Mullvad account number to configure the client with
        account: Option<String>,
    },

    /// Log out of the current account
    Logout,

    /// Display information about the current account
    Get {
        /// Enable verbose output
        #[arg(long, short = 'v')]
        verbose: bool,
    },

    /// Redeem a voucher
    Redeem {
        /// Voucher code to submit
        voucher: String,
    },
}

impl Account {
    pub async fn handle(self) -> Result<()> {
        let mut rpc = MullvadProxyClient::new().await?;
        match self {
            Account::Create => Self::create(&mut rpc).await,
            Account::Login { account } => {
                Self::login(
                    &mut rpc,
                    unwrap_or_from_stdin(account, "Enter an account number: ").await,
                )
                .await
            }
            Account::Logout => Self::logout(&mut rpc).await,
            Account::Get { verbose } => Self::get(&mut rpc, verbose).await,
            Account::Redeem { voucher } => Self::redeem_voucher(&mut rpc, voucher).await,
        }
    }

    async fn create(rpc: &mut MullvadProxyClient) -> Result<()> {
        rpc.create_new_account().await?;
        println!("New account created!");
        Self::get(rpc, false).await
    }

    async fn login(rpc: &mut MullvadProxyClient, account_number: AccountNumber) -> Result<()> {
        rpc.login_account(account_number.clone()).await?;
        println!("Warren account \"{account_number}\" set");
        Ok(())
    }

    async fn logout(rpc: &mut MullvadProxyClient) -> Result<()> {
        rpc.logout_account(&format!("{BIN_NAME} logout")).await?;
        println!("Removed device from Warren account");
        Ok(())
    }

    async fn get(rpc: &mut MullvadProxyClient, verbose: bool) -> Result<()> {
        let state = rpc.get_device().await?;

        match state {
            DeviceState::LoggedIn(identity) => {
                let pubkey = identity.pubkey.as_str().to_owned();
                println!("{:<20}{}", "Warren account:", pubkey);

                let data = rpc.get_account_data(pubkey).await?;
                println!(
                    "{:<20}{}",
                    "Expires at:",
                    data.expiry.with_timezone(&chrono::Local)
                );
                if verbose {
                    println!("{:<20}{}", "Account id:", data.id);
                }
            }
            DeviceState::LoggedOut => {
                println!("{NOT_LOGGED_IN_MESSAGE}");
            }
            DeviceState::Revoked => {
                println!("{REVOKED_MESSAGE}");
                if let Some(account_number) = rpc.get_account_history().await? {
                    println!("Warren account: {account_number}");
                }
            }
        }

        Ok(())
    }

    async fn redeem_voucher(rpc: &mut MullvadProxyClient, mut voucher: String) -> Result<()> {
        voucher.retain(|c| c.is_alphanumeric());

        let submission = rpc.submit_voucher(voucher).await?;
        println!(
            "Added {} to the account",
            format_duration(submission.time_added)
        );
        println!(
            "New expiry date: {}",
            submission.new_expiry.with_timezone(&chrono::Local),
        );
        Ok(())
    }
}

async fn unwrap_or_from_stdin(val: Option<String>, prompt_str: &'static str) -> String {
    if let Some(val) = val {
        return val;
    }

    tokio::task::spawn_blocking(|| from_stdin(prompt_str))
        .await
        .unwrap()
}

fn from_stdin(prompt_str: &'static str) -> String {
    let mut val = String::new();
    io::stdout()
        .write_all(prompt_str.as_bytes())
        .expect("Failed to write to STDOUT");
    let _ = io::stdout().flush();
    io::stdin()
        .read_line(&mut val)
        .expect("Failed to read from STDIN");
    val.split_whitespace().join("")
}

fn format_duration(seconds: u64) -> String {
    let dur = chrono::Duration::seconds(seconds as i64);
    if dur.num_days() > 0 {
        format!("{} days", dur.num_days())
    } else if dur.num_hours() > 0 {
        format!("{} hours", dur.num_hours())
    } else if dur.num_minutes() > 0 {
        format!("{} minutes", dur.num_minutes())
    } else {
        format!("{} seconds", dur.num_seconds())
    }
}
