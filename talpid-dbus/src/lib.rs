//! DBus system connection
#![cfg(target_os = "linux")]

#[cfg(feature = "libdbus")]
pub mod network_manager;
pub mod systemd;
pub mod systemd_resolved;

#[cfg(feature = "libdbus")]
pub use dbus;

#[cfg(feature = "libdbus")]
use std::sync::{Arc, LazyLock, Mutex};

/// Reuse or create a system DBus connection.
#[cfg(feature = "libdbus")]
pub fn get_connection() -> Result<Arc<dbus::blocking::SyncConnection>, dbus::Error> {
    use dbus::blocking::SyncConnection;

    static DBUS_CONNECTION: LazyLock<Mutex<Option<Arc<SyncConnection>>>> =
        LazyLock::new(|| Mutex::new(None));

    let mut connection = DBUS_CONNECTION.lock().expect("DBus lock poisoned");
    match &*connection {
        Some(existing_connection) => Ok(existing_connection.clone()),
        None => {
            let new_connection = Arc::new(SyncConnection::new_system()?);
            *connection = Some(new_connection.clone());
            Ok(new_connection)
        }
    }
}
