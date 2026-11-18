use crate::BIN_NAME;
use anyhow::{Context, Result};
use clap::Subcommand;
use mullvad_management_interface::MullvadProxyClient;
use mullvad_types::device::DeviceState;
use std::io::{self, BufRead, IsTerminal, Write};

const REVOKED_MESSAGE: &str = "The current device has been revoked";

// A Warren account IS a BIP39 recovery phrase (no account numbers): the phrase
// derives the Ed25519 identity and its `wb…` address. So `create` mints a fresh
// phrase and `login` restores from one; both go through the daemon's mnemonic
// signer (set_warren_mnemonic / create_new_account), never the legacy
// account-number path.
#[derive(Subcommand, Debug)]
pub enum Account {
    /// Create a brand-new Warren identity (generates a recovery phrase)
    Create,

    /// Restore a Warren identity from its recovery phrase (12 or 24 words),
    /// read from the terminal without echo, or from standard input when it is
    /// piped.
    Login,

    /// Log out (keeps the recovery phrase on this device)
    Logout,

    /// Display the current identity address and subscription
    Get {
        /// Enable verbose output
        #[arg(long, short = 'v')]
        verbose: bool,
    },

    /// Redeem a voucher to add subscription time.
    ///
    /// Without VOUCHER you are prompted for the code, or it is read from the
    /// first line of standard input. A code given as an argument is visible to
    /// every account on this machine for as long as the command runs.
    Redeem {
        /// Voucher code to submit (prefer the prompt or standard input)
        voucher: Option<String>,
    },

    /// Show the port-forwarding warnings on this account and any revocation,
    /// asked of the Warren API now.
    ///
    /// A forwarded port reported for abuse is closed and recorded as a
    /// warning, and enough warnings inside the window revoke the account
    /// (the output states both numbers). Each warning carries the case
    /// reference to quote when contesting it.
    Standing {
        /// Print the standing as one JSON object on one line.
        #[arg(long)]
        json: bool,
    },
}

impl Account {
    pub async fn handle(self) -> Result<()> {
        let mut rpc = MullvadProxyClient::new().await?;
        match self {
            Account::Create => Self::create(&mut rpc).await,
            Account::Login => Self::login(&mut rpc).await,
            Account::Logout => Self::logout(&mut rpc).await,
            Account::Get { verbose } => Self::get(&mut rpc, verbose).await,
            Account::Redeem { voucher } => Self::redeem_voucher(&mut rpc, voucher).await,
            Account::Standing { json } => Self::standing(&mut rpc, json).await,
        }
    }

    async fn standing(rpc: &mut MullvadProxyClient, json: bool) -> Result<()> {
        let standing = rpc
            .get_warren_account_standing()
            .await
            .context("failed to fetch the account standing")?;
        if json {
            println!(
                "{}",
                serde_json::to_string(&crate::standing::StandingJson::from(&standing))
                    .context("failed to format the account standing as JSON")?
            );
            return Ok(());
        }
        for line in crate::standing::report_lines(&standing) {
            println!("{line}");
        }
        Ok(())
    }

    async fn create(rpc: &mut MullvadProxyClient) -> Result<()> {
        let address = rpc
            .create_new_account()
            .await
            .context("failed to create a new Warren identity")?;
        // The new phrase is the ONLY way to recover this account, so fetch and
        // display it for the user to back up, on a terminal only.
        let on_terminal = io::stdout().is_terminal();
        let phrase = if crate::cmds::warren::may_print_phrase(on_terminal, false) {
            zeroize::Zeroizing::new(rpc.get_warren_mnemonic().await.unwrap_or_default())
        } else {
            zeroize::Zeroizing::new(String::new())
        };

        println!("New Warren identity created.");
        println!("{:<14}{address}", "Address:");
        if !on_terminal {
            println!();
            println!(
                "Standard output is not a terminal, so the recovery phrase is not printed.\n\
                 Back it up now from a terminal: {BIN_NAME} warren mnemonic export"
            );
        }
        if !phrase.is_empty() {
            println!();
            println!(
                "RECOVERY PHRASE: write it down offline. Anyone with it controls this\n\
                 account, and it is the only way to restore it. Never share it.\n"
            );
            println!("    {}\n", phrase.as_str());
        }
        println!("No subscription yet. Buy credit on the Warren website, then run:");
        println!("    {BIN_NAME} account redeem");
        println!("and type the voucher code when asked.");
        Ok(())
    }

    async fn login(rpc: &mut MullvadProxyClient) -> Result<()> {
        let input = tokio::task::spawn_blocking(crate::cmds::warren::read_phrase).await??;
        let phrase = crate::cmds::warren::phrase_from_input(&input);
        if phrase.is_empty() {
            anyhow::bail!("no recovery phrase provided");
        }
        rpc.set_warren_mnemonic(phrase.to_string()).await.context(
            "could not restore identity: the recovery phrase must be 12 or 24 BIP39 words",
        )?;
        println!("Warren identity restored and active.");
        Self::get(rpc, false).await
    }

    async fn logout(rpc: &mut MullvadProxyClient) -> Result<()> {
        rpc.logout_account(&format!("{BIN_NAME} logout"), false)
            .await?;
        println!(
            "Logged out. Your recovery phrase is kept on this device; restore later\n\
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
                        println!("{:<14}none, redeem a voucher to activate", "Subscription:");
                    }
                }
            }
            DeviceState::LoggedOut => {
                let phrase_kept = rpc.has_warren_identity().await?;
                for line in logged_out_lines(phrase_kept) {
                    println!("{line}");
                }
            }
            DeviceState::Revoked => {
                println!("{REVOKED_MESSAGE}");
            }
        }

        Ok(())
    }

    async fn redeem_voucher(rpc: &mut MullvadProxyClient, argument: Option<String>) -> Result<()> {
        if argument.is_none() && io::stdin().is_terminal() {
            eprint!("Voucher code: ");
            let _ = io::stderr().flush();
        }
        let (voucher, source) =
            tokio::task::spawn_blocking(move || read_voucher(argument, &mut io::stdin().lock()))
                .await??;
        if source == VoucherSource::Argument {
            eprintln!(
                "warning: a voucher code on the command line is visible to every account on this \
                 machine while the command runs. Run `{BIN_NAME} account redeem` without it to \
                 type it at a prompt, or pipe it on standard input."
            );
        }

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

/// What `account get` says on a logged-out device. A logout that keeps the
/// recovery phrase leaves it on the device, where `account create` would
/// replace it, so the two cases are not worded alike.
fn logged_out_lines(phrase_kept: bool) -> Vec<String> {
    if phrase_kept {
        vec![
            "Logged out. The recovery phrase is kept on this device.".to_owned(),
            format!("Log back in with `{BIN_NAME} account login` and that phrase."),
            format!("`{BIN_NAME} account create` would replace it with a new account."),
        ]
    } else {
        vec![
            "No Warren identity on this device".to_owned(),
            format!("Create one with `{BIN_NAME} account create`, or restore yours"),
            format!("with `{BIN_NAME} account login`."),
        ]
    }
}

/// Where a voucher code came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VoucherSource {
    Argument,
    Input,
}

/// The voucher code to redeem: the argument when there is one, otherwise the
/// first line of `input`, keeping only its letters and digits.
fn read_voucher(
    argument: Option<String>,
    input: &mut impl BufRead,
) -> Result<(String, VoucherSource)> {
    let (mut voucher, source) = match argument {
        Some(voucher) => (voucher, VoucherSource::Argument),
        None => {
            let mut line = String::new();
            input
                .read_line(&mut line)
                .context("failed to read the voucher code")?;
            (line, VoucherSource::Input)
        }
    };
    voucher.retain(|c| c.is_alphanumeric());
    if voucher.is_empty() {
        anyhow::bail!("no voucher code provided");
    }
    Ok((voucher, source))
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

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn a_logout_that_kept_the_phrase_says_so() {
        let text = logged_out_lines(true).join("\n");

        assert!(text.starts_with("Logged out."), "{text}");
        assert!(text.contains("phrase is kept on this device"), "{text}");
        assert!(!text.contains("No Warren identity"), "{text}");
    }

    #[test]
    fn a_device_with_no_phrase_says_there_is_no_identity() {
        let text = logged_out_lines(false).join("\n");

        assert!(
            text.starts_with("No Warren identity on this device"),
            "{text}"
        );
    }

    /// A phrase on the command line is visible to every local account in the
    /// process list and stays in the shell history: refused.
    #[test]
    fn login_never_takes_the_phrase_as_an_argument() {
        assert!(
            crate::Cli::try_parse_from([BIN_NAME, "account", "login", "abandon", "about"]).is_err()
        );
        assert!(crate::Cli::try_parse_from([BIN_NAME, "account", "login"]).is_ok());
    }

    #[test]
    fn redeem_takes_no_voucher_argument_to_read_it_from_input() {
        let parsed = crate::Cli::try_parse_from([BIN_NAME, "account", "redeem"]);

        assert!(parsed.is_ok());
    }

    #[test]
    fn a_voucher_typed_on_input_is_read_and_normalised() {
        let mut input = io::Cursor::new(b"abcd-1234 efgh\nignored\n".to_vec());

        let voucher = read_voucher(None, &mut input).unwrap();

        assert_eq!(
            voucher,
            (String::from("abcd1234efgh"), VoucherSource::Input)
        );
    }

    #[test]
    fn a_voucher_argument_is_used_without_reading_input() {
        let mut input = io::Cursor::new(b"other\n".to_vec());

        let voucher = read_voucher(Some("wxyz-9876".to_owned()), &mut input).unwrap();

        assert_eq!(voucher, (String::from("wxyz9876"), VoucherSource::Argument));
        assert_eq!(input.position(), 0);
    }

    #[test]
    fn an_empty_input_is_refused() {
        let mut input = io::Cursor::new(b" - \n".to_vec());

        let error = read_voucher(None, &mut input).unwrap_err();

        assert_eq!(error.to_string(), "no voucher code provided");
    }
}
