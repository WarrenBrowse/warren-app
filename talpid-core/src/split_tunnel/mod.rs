#[cfg(any(windows, test))]
mod driver_addresses;

/// Whether include-only may run on Windows. Not yet: the unmodified driver
/// soft-permits the apps it splits from every source address but its
/// "tunnel" one, so an included app binding a second interface's address
/// would leave outside the tunnel, and on beta and staging the driver cannot
/// reach winfw's salted sublayers at all (`docs/app-routing.md` §3.2).
#[cfg(windows)]
pub const INCLUDE_ONLY_READY: bool = false;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux as imp;

#[cfg(windows)]
#[path = "windows/mod.rs"]
mod imp;

#[cfg(target_os = "macos")]
#[path = "macos/mod.rs"]
mod imp;

#[cfg(target_os = "android")]
#[path = "android.rs"]
mod imp;

pub use imp::*;
