//! NetworkManager VPN service plugin for Warren.
//!
//! Warren builds its own tunnel: the engine opens the tun device, sets the
//! addresses and owns the routing. None of that is visible to a desktop,
//! because GNOME and KDE light their VPN indicator off one thing only, a
//! NetworkManager connection whose type is `vpn`. Only a registered VPN
//! service plugin can produce one.
//!
//! So this process describes a tunnel that already exists rather than
//! building one. It never touches the datapath: it reads the interface the
//! daemon named, reports its addresses back, and retracts the connection
//! when that interface goes away.

mod config;

#[cfg(target_os = "linux")]
mod iface;
#[cfg(target_os = "linux")]
mod logging;
#[cfg(target_os = "linux")]
mod service;

fn main() {
    #[cfg(target_os = "linux")]
    service::main();
}
