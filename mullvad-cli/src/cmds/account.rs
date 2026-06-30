use crate::BIN_NAME;
use crate::cmds::warren::normalize_mnemonic_words;
use anyhow::{Context, Result};
use clap::Subcommand;
use mullvad_management_interface::MullvadProxyClient;
use mullvad_types::device::DeviceState;
use std::io::{self, Write};

const NOT_LOGGED_IN_MESSAGE: &str = "No Warren identity on this device";
const REVOKED_MESSAGE: &str = "The current device has been revoked";

// A Warren account IS a BIP39 recovery phrase (no account numbers): the phrase
// derives the Ed25519 identity and its `wb…` address. So `create` mints a fresh
// phrase and `login` restores from one — both go through the daemon's mnemonic
// signer (set_warren_mnemonic / create_new_account), never the legacy
// account-number path.
#[derive(Subcommand, Debug)]
pub enum Account {
    /// Create a brand-new Warren identity (generates a recovery phrase)
    Create,

    /// Restore a Warren identity from its recovery phrase (12 or 24 words).
    ///
    /// Quote the whole phrase or pass the words as separate arguments; you are
    /// prompted for it if omitted.
    Login {
        #[arg(value_name = "WORD")]
        phrase: Vec<String>,
    },

    /// Log out (keeps the recovery phrase on this device)
    Logout,

    /// Display the current identity address and subscription
    Get {
        /// Enable verbose output
        #[arg(long, short = 'v')]
        verbose: bool,
    },

    /// Redeem a voucher to add subscription time
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
            Account::Login { phrase } => Self::login(&mut rpc, phrase).await,
            Account::Logout => Self::logout(&mut rpc).await,
            Account::Get { verbose } => Self::get(&mut rpc, verbose).await,
            Account::Redeem { voucher } => Self::redeem_voucher(&mut rpc, voucher).await,
        }
    }

    async fn create(rpc: &mut MullvadProxyClient) -> Result<()> {
        let address = rpc
            .create_new_account()
            .await
            .context("failed to create a new Warren identity")?;
        // The new phrase is the ONLY way to recover this account, so fetch and
        // display it for the user to back up.
        let phrase = rpc.get_warren_mnemonic().await.unwrap_or_default();

        println!("New Warren identity created.");
        println!("{:<14}{address}", "Address:");
        if !phrase.is_empty() {
            println!();
            println!(
                "RECOVERY PHRASE — write it down offline. Anyone with it controls this\n\
                 account, and it is the only way to restore it. Never share it.\n"
            );
            println!("    {phrase}\n");
        }
        println!("No subscription yet. Buy credit on the Warren website, then run:");
        println!("    {BIN_NAME} account redeem <VOUCHER>");
        Ok(())
    }

    async fn login(rpc: &mut MullvadProxyClient, phrase_words: Vec<String>) -> Result<()> {
        let phrase = if phrase_words.is_empty() {
            read_phrase_from_stdin().await
        } else {
            normalize_mnemonic_words(&phrase_words)
        };
        if phrase.is_empty() {
            anyhow::bail!("no recovery phrase provided");
        }
        rpc.set_warren_mnemonic(phrase)
            .await
            .context("could not restore identity — the recovery phrase must be 12 or 24 BIP39 words")?;
        println!("Warren identity restored and active.");
        Self::get(rpc, false).await
    }

    async fn logout(rpc: &mut MullvadProxyClient) -> Result<()> {
        rpc.logout_account(&format!("{BIN_NAME} logout")).await?;
        println!(
            "Logged out. Your recovery phrase is kept on this device — restore later\n\
             with `{BIN_NAME} account login`."
        );
        Ok(())
    }

    async fn get(rpc: &mut MullvadProxyClient, verbose: bool) -> Result<()> {
        let state = rpc.get_device().await?;

        match state {
            DeviceState::LoggedIn(identity) => {
                let pubkey = identity.pubkey.as_str().to_owned();
                println!("{:<14}{pubkey}", "Address:");

                match rpc.get_account_data(pubkey).await {
                    Ok(data) => {
                        println!(
                            "{:<14}active, expires {}",
                            "Subscription:",
                            data.expiry.with_timezone(&chrono::Local)
                        );
                        if verbose {
                            println!("{:<14}{}", "Account id:", data.id);
                        }
                    }
                    // No subscription bound to this identity yet (warren-api 404),
                    // or the API was unreachable. Either way, nothing to connect with.
                    Err(_) => {
                        println!(
                            "{:<14}none — redeem a voucher to activate",
                            "Subscription:"
                        );
                    }
                }
            }
            DeviceState::LoggedOut => {
                println!("{NOT_LOGGED_IN_MESSAGE}");
                println!("Create one with `{BIN_NAME} account create`, or restore yours");
                println!("with `{BIN_NAME} account login`.");
            }
            DeviceState::Revoked => {
                println!("{REVOKED_MESSAGE}");
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

/// Prompts for and reads a recovery phrase as a single line (words stay
/// space-separated — never concatenated). Normalised to the canonical lowercase
/// single-spaced form the BIP39 validator expects.
async fn read_phrase_from_stdin() -> String {
    tokio::task::spawn_blocking(|| {
        io::stdout()
            .write_all(b"Enter your recovery phrase (12 or 24 words): ")
            .expect("Failed to write to STDOUT");
        let _ = io::stdout().flush();
        let mut line = String::new();
        io::stdin()
            .read_line(&mut line)
            .expect("Failed to read from STDIN");
        normalize_mnemonic_words(&[line])
    })
    .await
    .unwrap()
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
