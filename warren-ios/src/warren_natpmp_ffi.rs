//! Warren NAT-PMP FFI for iOS (port forwarding via warrenguard-natpmp-client).
//!
//! Skeleton: implementations land with the NAT-PMP UI settings + status.
//! The Swift side wraps these in a `WarrenPortForwarding` actor that
//! surfaces the forwarded port + lifetime to the user (a
//! Warren product differentiator vs Mullvad/IVPN abandon 2023).
//!
//! Intended exports:
//! - `warren_natpmp_request_mapping(handle: *mut WarrenTunnelHandle, internal_port: u16, lifetime_seconds: u32, out_mapping: *mut WarrenNatPmpMapping)`
//! - `warren_natpmp_renew_mapping(mapping: *mut WarrenNatPmpMapping)`
//! - `warren_natpmp_release_mapping(mapping: *mut WarrenNatPmpMapping)`
//! - `warren_natpmp_set_event_callback(callback, context)` for Mapped /
//!   Renewed / Failed / Cancelled events (cf. the
//!   `refresh_loop`)
//!
//! Underlying crates (path-deps to add when wiring):
//! - `warrenguard-natpmp-client` (mpsc events Mapped / Renewed / Failed / Cancelled)
//! - `warrenguard-natpmp-protocol` (wire format)
//!
//! Note: NAT-PMP requires the tunnel to be established first
//! (the mapping is requested from the exit's gateway via the tunnel).
//! Therefore `WarrenTunnelHandle` is a required parameter.
