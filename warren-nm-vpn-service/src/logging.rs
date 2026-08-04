//! Logging to stderr, which NetworkManager captures into the journal
//! alongside its own VPN lines. The plugin is spawned by NetworkManager and
//! owns no log file of its own, so there is nowhere else worth writing.

use log::{Level, LevelFilter, Metadata, Record};

/// Nothing here is secret: the plugin only ever sees an interface name and
/// the peer address the daemon already published, both of which the user
/// can read from their own routing table. No identity material passes
/// through this process, so there is nothing to redact.
struct StderrLogger;

impl log::Log for StderrLogger {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.level() <= Level::Debug
    }

    fn log(&self, record: &Record<'_>) {
        if self.enabled(record.metadata()) {
            eprintln!(
                "[warren-nm-vpn-service] {} {}",
                record.level(),
                record.args()
            );
        }
    }

    fn flush(&self) {}
}

static LOGGER: StderrLogger = StderrLogger;

pub fn init() {
    if log::set_logger(&LOGGER).is_ok() {
        log::set_max_level(LevelFilter::Debug);
    }
}
