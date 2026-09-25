//! Port-forward abuse standing on iOS (warren-core doc 105 §5.3, §5.4).
//!
//! iOS runs two processes, and each holds its own
//! [`warren_standing::StandingStore`]:
//! - the Network Extension's store learns a ban from the token issuer's
//!   refusal (the background refresh in `warren_token_provider`) and from an
//!   exit's ban rejection, and the tunnel reads it before it dials and while it
//!   runs. It keeps no strike ledger: the extension announces no strike.
//! - the app's store answers the standing poll (`warren_account_standing`)
//!   and remembers which strikes it already announced in a ledger in the
//!   app's own container, as digests only.
//!
//! The store logic, the envelope and their tests live in `warren-standing`,
//! shared with Android, so the two mobile clients cannot drift on either.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock, PoisonError};

use warren_standing::{LEDGER_FILE, StandingStore};

static TUNNEL_STORE: OnceLock<StandingStore> = OnceLock::new();

/// The Network Extension's store.
#[cfg_attr(
    not(all(target_os = "ios", feature = "tunnel")),
    expect(dead_code, reason = "read by the iOS tunnel only")
)]
pub(crate) fn tunnel_store() -> &'static StandingStore {
    TUNNEL_STORE.get_or_init(|| StandingStore::new(None))
}

/// The app's stores, one per ledger directory: in practice one, the app's
/// own container. Leaked on purpose: a store lives as long as the process.
static APP_STORES: Mutex<Vec<(PathBuf, &'static StandingStore)>> = Mutex::new(Vec::new());

/// The app's store whose ledger lives in `ledger_dir`.
pub(crate) fn app_store(ledger_dir: &Path) -> &'static StandingStore {
    let mut stores = APP_STORES.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some((_, store)) = stores.iter().find(|(dir, _)| dir == ledger_dir) {
        return store;
    }
    let store: &'static StandingStore = Box::leak(Box::new(StandingStore::new(Some(
        ledger_dir.join(LEDGER_FILE),
    ))));
    stores.push((ledger_dir.to_path_buf(), store));
    store
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_ledger_directory_is_one_store() {
        let dir = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();

        let first = app_store(dir.path());

        assert!(std::ptr::eq(first, app_store(dir.path())));
        assert!(!std::ptr::eq(first, app_store(other.path())));
    }
}
