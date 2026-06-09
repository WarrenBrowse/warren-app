//! CLI subcommands for Warren-specific settings: the warren-api server
//! URL and the BIP39 identity (recovery phrase) backup/restore.

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

    /// Unset the warren-api URL (restart daemon to apply, fallback Mullvad upstream)
    Unset,
}

#[derive(Subcommand, Debug)]
pub enum WarrenMnemonic {
    /// Print the current Warren recovery phrase so it can be backed up.
    /// WARNING: anyone with this phrase controls your identity and its
    /// subscription. Write it down offline; never share it.
    Export,

    /// Restore a Warren identity from a BIP39 recovery phrase. This
    /// REPLACES the current identity (the previous one becomes
    /// unrecoverable unless separately backed up). The daemon validates
    /// the phrase and hot-swaps the signer (no restart needed).
    Import {
        /// The recovery phrase (12 or 24 words, space-separated). Quote
        /// the whole phrase or pass the words as separate arguments.
        #[arg(required = true, num_args = 1..)]
        words: Vec<String>,
    },
}

/// Normalises a BIP39 phrase entered on the CLI: splits on any
/// whitespace (so a single quoted argument and separate word arguments
/// behave identically), lowercases each word (the BIP39 English
/// wordlist is lowercase), and rejoins with single spaces. Mirrors the
/// GUI `RestoreMnemonicView` normalisation so both entry paths derive
/// the same identity from the same phrase.
fn normalize_mnemonic_words(words: &[String]) -> String {
    words
        .iter()
        .flat_map(|w| w.split_whitespace())
        .map(str::to_lowercase)
        .collect::<Vec<_>>()
        .join(" ")
}

impl Warren {
    pub async fn handle(self) -> Result<()> {
        match self {
            Warren::ApiUrl(WarrenApiUrl::Get) => Self::api_url_get().await,
            Warren::ApiUrl(WarrenApiUrl::Set { url }) => Self::api_url_set(Some(url)).await,
            Warren::ApiUrl(WarrenApiUrl::Unset) => Self::api_url_set(None).await,
            Warren::Mnemonic(WarrenMnemonic::Export) => Self::mnemonic_export().await,
            Warren::Mnemonic(WarrenMnemonic::Import { words }) => {
                Self::mnemonic_import(&words).await
            }
        }
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
                println!("Warren api URL persisted: {u} (restart `mullvad-daemon` to apply)")
            }
            None => println!("Warren api URL unset (restart `mullvad-daemon` to apply)"),
        }
        Ok(())
    }

    async fn mnemonic_export() -> Result<()> {
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

    async fn mnemonic_import(words: &[String]) -> Result<()> {
        let phrase = normalize_mnemonic_words(words);
        let mut rpc = MullvadProxyClient::new().await?;
        rpc.set_warren_mnemonic(phrase).await?;
        println!("Warren identity restored - the new identity is now active.");
        println!(
            "If it has an active subscription you can connect; otherwise redeem a voucher first."
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_mnemonic_words;

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
