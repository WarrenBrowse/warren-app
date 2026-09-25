//! Port-forward abuse standing of the wallet on Android (warren-core doc 105):
//! the process-lived [`warren_standing::StandingStore`] every input of this
//! crate feeds (the standing poll, the issuers' ban refusals, an exit's ban
//! rejection) and the tunnel reads before it dials and while it runs. Its
//! strike ledger lives in the app files directory (`allowBackup=false`).

use std::sync::OnceLock;

use warren_standing::{LEDGER_FILE, StandingStore};

static STORE: OnceLock<StandingStore> = OnceLock::new();

/// Builds the store with its ledger in the app files directory. Called from
/// `initLogger`, the only JNI call that carries the directory; the first
/// caller wins.
pub(crate) fn init(files_dir: &std::path::Path) {
    let _ = STORE.set(StandingStore::new(Some(files_dir.join(LEDGER_FILE))));
}

/// The process store. Without `initLogger` it keeps the ledger in memory,
/// which costs a repeated warning after a process death and nothing else.
pub(crate) fn store() -> &'static StandingStore {
    STORE.get_or_init(|| StandingStore::new(None))
}
