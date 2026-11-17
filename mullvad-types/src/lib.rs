pub mod access_method;
pub mod account;
pub mod app_routing;
pub mod auth_failed;
pub mod constraints;
pub mod custom_list;
pub mod device;
pub mod endpoint;
pub mod features;
pub mod location;
pub mod relay_constraints;
pub mod relay_list;
pub mod relay_selector;
pub mod settings;
pub mod states;
pub mod version;
pub mod warren_diagnostics;
pub mod warren_identity;
pub mod warren_pubkey;
pub mod wireguard;

mod custom_tunnel;
pub use crate::custom_tunnel::*;

// This environment's own, so co-installed products never share a routing table or a mark.
#[cfg(target_os = "linux")]
pub const TUNNEL_TABLE_ID: u32 = warren_product_env::CURRENT.linux_tunnel_fwmark();
#[cfg(target_os = "linux")]
pub const TUNNEL_FWMARK: u32 = warren_product_env::CURRENT.linux_tunnel_fwmark();

pub use constraints::Intersection;
pub use intersection_derive::Intersection;
