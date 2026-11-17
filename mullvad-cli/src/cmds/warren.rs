//! CLI subcommands for Warren-specific settings: the warren-api server
//! URL, the BIP39 identity (recovery phrase) backup/restore, and the
//! parallel QUIC connection count (performance tuning).

use anyhow::Result;
use clap::Subcommand;
use mullvad_management_interface::MullvadProxyClient;

#[derive(Subcommand, Debug)]
pub enum Warren {
    /// Manage the warren-api server URL (consumed by remote backends)
    #[clap(subcommand)]
    ApiUrl(WarrenApiUrl),

    /// Back up or restore the Warren identity (BIP39 recovery phrase)
    #[clap(subcommand)]
    Mnemonic(WarrenMnemonic),

    /// Tune the number of parallel QUIC connections (1-16)
    #[clap(subcommand)]
    NConnections(WarrenNConnections),

    /// Cap the tunnel's own bandwidth client-side (both directions)
    #[clap(subcommand)]
    MaxRate(WarrenMaxRate),
}

#[derive(Subcommand, Debug)]
pub enum WarrenMaxRate {
    /// Show the persisted cap ("<unset>" = unlimited)
    Get,

    /// Persist a cap in Mbps. Each direction is budgeted the full
    /// rate independently. Applies to a live tunnel immediately.
    Set {
        /// Cap in megabits per second (1-10000)
        #[arg(value_parser = clap::value_parser!(u32).range(1..=10_000))]
        mbps: u32,
    },

    /// Remove the cap (unlimited)
    Unset,
}

#[derive(Subcommand, Debug)]
pub enum WarrenNConnections {
    /// Show the persisted connection count ("<unset>" = daemon default)
    Get,

    /// Persist the connection count. Applied on the next (re)connect;
    /// the daemon reconnects automatically when the tunnel is up. The
    /// env var WARREN_N_CONNECTIONS on the daemon takes priority over
    /// this setting.
    Set {
        /// Parallel QUIC connections (1-16). Throughput plateaus at 8
        /// (the default); lower values reduce per-client footprint.
        #[arg(value_parser = clap::value_parser!(u8).range(1..=16))]
        n: u8,
    },

    /// Reset to the daemon's compiled default (8)
    Reset,
}

#[derive(Subcommand, Debug)]
pub enum WarrenApiUrl {
    /// Show the persisted warren-api URL (or "<unset>" if absent)
    Get,

    /// Persist the warren-api URL (restart daemon to apply)
    Set {
        /// Format `http(s)://host:port` without trailing slash
        url: String,
    },

    /// Unset the warren-api URL (restart daemon to apply, falls back to the API of this build's
    /// channel)
    Unset,
}

#[derive(Subcommand, Debug)]
pub enum WarrenMnemonic {
    /// Print the current Warren recovery phrase so it can be backed up.
    /// WARNING: anyone with this phrase controls your identity and its
    /// subscription. Write it down offline; never share it.
    ///
    /// The phrase is printed to a terminal only, unless --to-pipe allows a
    /// pipe or a file.
    Export {
        /// Also print the phrase when standard output is a pipe or a file.
        #[arg(long)]
        to_pipe: bool,
    },

    /// Restore a Warren identity from a BIP39 recovery phrase, read from
    /// the terminal without echo, or from standard input when it is piped.
    /// This REPLACES the current identity (the previous one becomes
    /// unrecoverable unless separately backed up). The daemon validates
    /// the phrase and hot-swaps the signer (no restart needed).
    Import,
}

/// Normalises a BIP39 phrase entered on the CLI: splits on any
/// whitespace (so a single quoted argument and separate word arguments
/// behave identically), lowercases each word (the BIP39 English
/// wordlist is lowercase), and rejoins with single spaces. Mirrors the
/// GUI `RestoreMnemonicView` normalisation so both entry paths derive
/// the same identity from the same phrase.
pub(crate) fn normalize_mnemonic_words(words: &[String]) -> String {
    words
        .iter()
        .flat_map(|w| w.split_whitespace())
        .map(str::to_lowercase)
        .collect::<Vec<_>>()
        .join(" ")
}

/// The phrase as entered, wiped when dropped. Typed at a terminal it is read
/// without echo; piped, the whole input is read.
pub(crate) fn read_phrase() -> Result<zeroize::Zeroizing<String>> {
    use std::io::{IsTerminal, Read};
    if std::io::stdin().is_terminal() {
        let typed = rpassword::prompt_password("Recovery phrase (input hidden): ")?;
        Ok(zeroize::Zeroizing::new(typed))
    } else {
        let mut piped = zeroize::Zeroizing::new(String::new());
        std::io::stdin().read_to_string(&mut piped)?;
        Ok(piped)
    }
}

/// Whether the recovery phrase may be written to standard output: always to a
/// terminal, to a pipe or a file only when the user asked for it, since that
/// output often ends up in a log or a shared file.
pub(crate) fn may_print_phrase(stdout_is_terminal: bool, to_pipe: bool) -> bool {
    stdout_is_terminal || to_pipe
}

/// [`normalize_mnemonic_words`] over one entered block of text.
pub(crate) fn phrase_from_input(input: &str) -> zeroize::Zeroizing<String> {
    zeroize::Zeroizing::new(normalize_mnemonic_words(&[input.to_owned()]))
}

impl Warren {
    pub async fn handle(self) -> Result<()> {
        match self {
            Warren::ApiUrl(WarrenApiUrl::Get) => Self::api_url_get().await,
            Warren::ApiUrl(WarrenApiUrl::Set { url }) => Self::api_url_set(Some(url)).await,
            Warren::ApiUrl(WarrenApiUrl::Unset) => Self::api_url_set(None).await,
            Warren::Mnemonic(WarrenMnemonic::Export { to_pipe }) => {
                Self::mnemonic_export(to_pipe).await
            }
            Warren::Mnemonic(WarrenMnemonic::Import) => Self::mnemonic_import().await,
            Warren::NConnections(WarrenNConnections::Get) => Self::n_connections_get().await,
            Warren::NConnections(WarrenNConnections::Set { n }) => {
                Self::n_connections_set(Some(n)).await
            }
            Warren::NConnections(WarrenNConnections::Reset) => Self::n_connections_set(None).await,
            Warren::MaxRate(WarrenMaxRate::Get) => Self::max_rate_get().await,
            Warren::MaxRate(WarrenMaxRate::Set { mbps }) => {
                Self::max_rate_set(Some(u64::from(mbps) * 1_000_000)).await
            }
            Warren::MaxRate(WarrenMaxRate::Unset) => Self::max_rate_set(None).await,
        }
    }

    async fn max_rate_get() -> Result<()> {
        let mut rpc = MullvadProxyClient::new().await?;
        let settings = rpc.get_settings().await?;
        match settings.warren_max_rate_bps {
            Some(bps) => println!("Warren max rate: {} Mbps ({bps} bps)", bps / 1_000_000),
            None => println!("Warren max rate: <unset> (unlimited)"),
        }
        Ok(())
    }

    async fn max_rate_set(bps: Option<u64>) -> Result<()> {
        let mut rpc = MullvadProxyClient::new().await?;
        rpc.set_warren_max_rate_bps(bps).await?;
        match bps {
            Some(bps) => println!(
                "Warren max rate set to {} Mbps (applies to the live tunnel immediately)",
                bps / 1_000_000
            ),
            None => println!("Warren max rate unset (unlimited)"),
        }
        Ok(())
    }

    async fn n_connections_get() -> Result<()> {
        let mut rpc = MullvadProxyClient::new().await?;
        let settings = rpc.get_settings().await?;
        match settings.warren_n_connections {
            Some(n) => println!("Warren n_connections: {n}"),
            None => println!("Warren n_connections: <unset> (daemon default, 8)"),
        }
        Ok(())
    }

    async fn n_connections_set(n: Option<u8>) -> Result<()> {
        let mut rpc = MullvadProxyClient::new().await?;
        rpc.set_warren_n_connections(n).await?;
        match n {
            Some(n) => println!(
                "Warren n_connections set to {n} (reconnects automatically if the tunnel is up)"
            ),
            None => println!("Warren n_connections reset to the daemon default"),
        }
        Ok(())
    }

    async fn api_url_get() -> Result<()> {
        let mut rpc = MullvadProxyClient::new().await?;
        let settings = rpc.get_settings().await?;
        match settings.warren_api_url {
            Some(url) => println!("Warren api URL: {url}"),
            None => println!("Warren api URL: <unset>"),
        }
        Ok(())
    }

    async fn api_url_set(url: Option<String>) -> Result<()> {
        let mut rpc = MullvadProxyClient::new().await?;
        rpc.set_warren_api_url(url.clone()).await?;
        match url {
            Some(u) => {
                println!("Warren api URL persisted: {u} (restart the Warren daemon to apply)")
            }
            None => println!("Warren api URL unset (restart the Warren daemon to apply)"),
        }
        Ok(())
    }

    async fn mnemonic_export(to_pipe: bool) -> Result<()> {
        use std::io::IsTerminal;
        if !may_print_phrase(std::io::stdout().is_terminal(), to_pipe) {
            anyhow::bail!(
                "Standard output is not a terminal: the recovery phrase is not printed. \
                 Run the command in a terminal, or pass --to-pipe to write it to this pipe or file."
            );
        }
        let mut rpc = MullvadProxyClient::new().await?;
        let mnemonic = rpc.get_warren_mnemonic().await?;
        if mnemonic.is_empty() {
            println!("No Warren identity has been created yet - nothing to export.");
            return Ok(());
        }
        println!(
            "WARNING: anyone with this recovery phrase controls your Warren identity and its \
             subscription."
        );
        println!("Write it down offline. Never share it or store it in the cloud.\n");
        println!("{mnemonic}");
        Ok(())
    }

    async fn mnemonic_import() -> Result<()> {
        let input = read_phrase()?;
        let phrase = phrase_from_input(&input);
        if phrase.is_empty() {
            anyhow::bail!("No recovery phrase was entered.");
        }
        let mut rpc = MullvadProxyClient::new().await?;
        rpc.set_warren_mnemonic(phrase.to_string()).await?;
        println!("Warren identity restored - the new identity is now active.");
        println!(
            "If it has an active subscription you can connect; otherwise redeem a voucher first."
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{Warren, may_print_phrase, normalize_mnemonic_words, phrase_from_input};
    use clap::Parser;

    #[derive(Parser, Debug)]
    struct Probe {
        #[command(subcommand)]
        warren: Warren,
    }

    /// A phrase on the command line is visible to every local account in
    /// the process list and stays in the shell history: refused.
    #[test]
    fn the_phrase_is_never_taken_as_an_argument() {
        assert!(
            Probe::try_parse_from(["probe", "mnemonic", "import", "abandon", "about"]).is_err()
        );
        assert!(Probe::try_parse_from(["probe", "mnemonic", "import"]).is_ok());
    }

    #[test]
    fn the_phrase_reaches_a_pipe_only_when_asked() {
        assert!(may_print_phrase(true, false));
        assert!(!may_print_phrase(false, false));
        assert!(may_print_phrase(false, true));
        assert!(Probe::try_parse_from(["probe", "mnemonic", "export", "--to-pipe"]).is_ok());
    }

    /// Piped input (a file, a password manager) reads like typed input.
    #[test]
    fn a_piped_phrase_is_normalised_like_a_typed_one() {
        assert_eq!(
            phrase_from_input("  Legal WINNER\n\tthank \n").as_str(),
            "legal winner thank"
        );
    }

    #[test]
    fn normalize_collapses_whitespace_and_lowercases() {
        // A user may paste the phrase as one quoted argument with
        // irregular spacing and mixed case. Normalisation must produce
        // the canonical lowercase single-spaced form the BIP39
        // validator (and the GUI restore path) expect - otherwise a
        // perfectly valid phrase would be rejected.
        let words = vec!["  Abandon   ABILITY ".to_owned(), "able\tABOUT".to_owned()];
        assert_eq!(
            normalize_mnemonic_words(&words),
            "abandon ability able about",
            "phrase must be lowercased and whitespace-collapsed regardless of how it was typed"
        );
    }

    #[test]
    fn normalize_handles_one_word_per_argument() {
        // The other entry style: each word as its own CLI argument.
        let words = vec!["legal".to_owned(), "winner".to_owned(), "thank".to_owned()];
        assert_eq!(normalize_mnemonic_words(&words), "legal winner thank");
    }
}
