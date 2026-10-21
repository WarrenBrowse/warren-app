//! Where an install's sessions start in each epoch's token batch, kept across
//! process restarts (`TokenManager::with_rotation`).
//!
//! The exit recognises a restarted client only by the serial it leads with,
//! and keeps its inner address and forwarded ports for that serial. A start
//! drawn per process leads a restarted daemon or tunnel extension with another
//! serial, so its pinned port stays refused for minutes after every reboot or
//! update. Shared by the desktop daemon, Android and iOS.

use std::path::Path;

use warren_api::{HttpTransport, TokenManager};

/// File under a client's state directory that keeps [`install_rotation`]'s
/// value across process restarts.
pub const ROTATION_FILE: &str = "warren-token-rotation";

/// This install's rotation: read from `dir`, or drawn and kept there on first
/// use. In a directory it cannot write to, each process draws its own. The
/// value is written to a side file and renamed, because a torn write would
/// still parse as another number and silently move the next start.
pub fn install_rotation(dir: &Path) -> u64 {
    let path = dir.join(ROTATION_FILE);
    if let Some(kept) = std::fs::read_to_string(&path)
        .ok()
        .and_then(|text| text.trim().parse().ok())
    {
        return kept;
    }
    let drawn: u64 = rand::random();
    let partial = dir.join(format!("{ROTATION_FILE}.partial"));
    if let Err(e) =
        std::fs::write(&partial, drawn.to_string()).and_then(|()| std::fs::rename(&partial, &path))
    {
        log::warn!("Warren token rotation not kept, a restart will lead with another serial: {e}");
    }
    drawn
}

/// `manager`, leading its sessions with [`install_rotation`]'s value for
/// `dir`. Without a directory it keeps the start it drew for this process.
#[must_use]
pub fn with_install_rotation<T: HttpTransport>(
    manager: TokenManager<T>,
    dir: Option<&Path>,
) -> TokenManager<T> {
    match dir {
        Some(dir) => manager.with_rotation(install_rotation(dir)),
        None => manager,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use data_encoding::BASE64URL_NOPAD;
    use warren_api::{BlindingKey, PersistedTokens, TokenManager, WarrenApiClient};
    use warren_identity::WarrenIdentity;

    struct NoNetwork;

    impl warren_api::HttpTransport for NoNetwork {
        async fn execute(
            &self,
            _request: warren_api::HttpRequest,
        ) -> Result<warren_api::HttpResponse, warren_api::TransportError> {
            panic!("these tests never reach the issuer")
        }
    }

    const NOW: u64 = 100 * 3600 + 5;

    fn bare_manager() -> TokenManager<NoNetwork> {
        let seed = [0x42; 32];
        TokenManager::new(
            Arc::new(WarrenApiClient::new(
                "https://api.example.test",
                WarrenIdentity::from_seed(&seed),
                NoNetwork,
            )),
            BlindingKey::session(&seed),
        )
    }

    /// Two hundred tokens for epoch 100, so two independent draws lead with
    /// the same one only once in two hundred runs.
    fn restore(manager: &TokenManager<NoNetwork>) {
        let batch: Vec<String> = (1..=200u8)
            .map(|fill| {
                let mut token = [fill; warrenguard_token::TOKEN_LEN];
                token[..2].copy_from_slice(&2u16.to_be_bytes());
                BASE64URL_NOPAD.encode(&token)
            })
            .collect();
        let bundle = serde_json::json!({ "epoch_secs": 3600, "epochs": { "100": batch } });
        let _ = manager
            .restore_persisted(&PersistedTokens::from_json(&bundle.to_string()).expect("a bundle"));
    }

    #[test]
    fn the_next_process_reads_back_the_rotation_this_one_drew() {
        let dir = tempfile::tempdir().expect("a temp dir");

        let drawn = super::install_rotation(dir.path());

        assert_eq!(super::install_rotation(dir.path()), drawn);
    }

    #[test]
    fn an_unreadable_rotation_file_is_drawn_again_and_kept() {
        let dir = tempfile::tempdir().expect("a temp dir");
        std::fs::write(dir.path().join(super::ROTATION_FILE), b"not a number").unwrap();

        let drawn = super::install_rotation(dir.path());

        assert_eq!(super::install_rotation(dir.path()), drawn);
    }

    #[test]
    fn a_rotation_that_cannot_be_kept_is_still_handed_out() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let missing = dir.path().join("never-created");

        let one = super::install_rotation(&missing);
        let other = super::install_rotation(&missing);

        assert_ne!(
            one, other,
            "nothing was kept, so each process draws its own"
        );
    }

    #[test]
    fn a_manager_kept_over_an_install_leads_where_its_predecessor_led() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let lead = || {
            let manager = super::with_install_rotation(bare_manager(), Some(dir.path()));
            restore(&manager);
            manager.session_stack(NOW)[0]
        };

        assert_eq!(lead(), lead());
    }
}
