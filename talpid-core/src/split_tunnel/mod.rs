#[cfg(any(windows, test))]
mod driver_addresses;
#[cfg(any(windows, test))]
pub mod include_hold;

/// Whether include-only may run on Windows. The unmodified driver
/// soft-permits the apps it splits from every source address but its
/// "tunnel" one; winfw's hold (`include_hold`) is what keeps an included app
/// binding another address inside the tunnel (`docs/app-routing.md` §3.2).
#[cfg(windows)]
pub const INCLUDE_ONLY_READY: bool = true;

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
