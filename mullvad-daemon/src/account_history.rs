use mullvad_types::account::AccountNumber;
use mullvad_types::warren_pubkey::WarrenPubKey;
use std::{path::Path, str::FromStr};
use talpid_types::ErrorExt;
use tokio::{
    fs,
    io::{self, AsyncReadExt, AsyncSeekExt, AsyncWriteExt},
};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Unable to open or read account history file")]
    Read(#[source] io::Error),

    #[error("Failed to serialize account history")]
    Serialize(#[source] serde_json::Error),

    #[error("Unable to write account history file")]
    Write(#[source] io::Error),

    #[error("Write task panicked or was cancelled")]
    WriteCancelled(#[source] tokio::task::JoinError),
}

static ACCOUNT_HISTORY_FILE: &str = "account-history.json";

pub struct AccountHistory {
    file: io::BufWriter<fs::File>,
    number: Option<AccountNumber>,
}

/// Returns `true` if `token` is a recognised account-history entry.
///
/// Warren persists the account number as a `WarrenPubKey` SS58 address
/// (`wb…`, 47-49 base58 chars); that is the only format written today.
/// The legacy upstream numeric account number is still accepted so a
/// pre-fork history file migrates cleanly. The empty string is handled
/// by callers, not here.
///
/// Do NOT narrow this back to a fixed-width hex rule: `WarrenPubKey`'s
/// real representation is a base58 SS58 address, and a `[0-9a-fA-F]{64}`
/// regex rejected every live account, resetting account-history.json on
/// every boot.
pub(crate) fn is_known_account_number(token: &str) -> bool {
    WarrenPubKey::from_str(token).is_ok()
        || (!token.is_empty() && token.bytes().all(|b| b.is_ascii_digit()))
}

impl AccountHistory {
    pub async fn new(
        settings_dir: &Path,
        current_number: Option<AccountNumber>,
    ) -> Result<AccountHistory> {
        let mut options = fs::OpenOptions::new();
        cfg_select! {
            unix    => { options.mode(0o600); }
            windows => {
                // a share mode of zero ensures exclusive access to the file to *this* process
                options.share_mode(0);
            }
            _ => {}
        }

        let path = settings_dir.join(ACCOUNT_HISTORY_FILE);
        log::info!("Opening account history file in {}", path.display());
        let mut reader = options
            .write(true)
            .create(true)
            .read(true)
            .open(path)
            .await
            .map(io::BufReader::new)
            .map_err(Error::Read)?;

        let mut buffer = String::new();
        let (number, should_save): (Option<AccountNumber>, bool) =
            match reader.read_to_string(&mut buffer).await {
                Ok(_) if is_known_account_number(buffer.trim()) => {
                    // Trim trailing newline/whitespace before storing.
                    (Some(buffer.trim().to_string()), false)
                }
                Ok(0) => (current_number, true),
                Ok(_) | Err(_) => {
                    // Not a fatal condition: we fall back to
                    // `current_number` (= whatever device.json says is
                    // active) and rewrite the file on save. Both the
                    // Warren SS58 pubkey and the legacy numeric account
                    // number pass `is_known_account_number`, so this
                    // branch only fires on a genuinely garbled file -
                    // worth logging at INFO rather than WARN since the
                    // recovery is silent and automatic.
                    log::info!(
                        "account-history.json content does not match any known \
                         format; resetting from device state"
                    );
                    (current_number, true)
                }
            };

        let file = io::BufWriter::new(reader.into_inner());
        let mut history = AccountHistory { file, number };
        if should_save && let Err(error) = history.save_to_disk().await {
            log::error!(
                "{}",
                error.display_chain_with_msg("Failed to save account history after opening it")
            );
        }
        Ok(history)
    }

    /// Gets the account number in the history
    pub fn get(&self) -> Option<AccountNumber> {
        self.number.clone()
    }

    /// Replace the account number in the history
    pub async fn set(&mut self, new_entry: AccountNumber) -> Result<()> {
        self.number = Some(new_entry);
        self.save_to_disk().await
    }

    /// Remove account history
    pub async fn clear(&mut self) -> Result<()> {
        self.number = None;
        self.save_to_disk().await
    }

    async fn save_to_disk(&mut self) -> Result<()> {
        self.file.get_mut().set_len(0).await.map_err(Error::Write)?;
        self.file
            .seek(io::SeekFrom::Start(0))
            .await
            .map_err(Error::Write)?;
        if let Some(ref number) = self.number {
            self.file
                .write_all(number.as_bytes())
                .await
                .map_err(Error::Write)?;
        }
        self.file.flush().await.map_err(Error::Write)?;
        self.file.get_mut().sync_all().await.map_err(Error::Write)
    }
}
