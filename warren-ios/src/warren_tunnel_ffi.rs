//! Warren tunnel FFI for iOS (Quinn-based VPN tunnel via warren-tunnel).
//!
//! Skeleton: implementations land in C.4 (PacketTunnelProvider replacing
//! WireGuardAdapter). The Swift side wraps these in a `WarrenQuinnAdapter`
//! consumed by `PacketTunnel/PacketTunnelProvider`.
//!
//! Intended exports:
//! - `warren_tunnel_start(config: *const WarrenTunnelParameters, packet_fd: i32) -> *mut WarrenTunnelHandle`
//! - `warren_tunnel_stop(handle: *mut WarrenTunnelHandle)`
//! - `warren_tunnel_reconnect(handle: *mut WarrenTunnelHandle)` (Wi-Fi <-> cellular handover)
//! - `warren_tunnel_status(handle: *mut WarrenTunnelHandle, out: *mut WarrenTunnelStatus)`
//! - `warren_tunnel_set_event_callback(handle, callback, context)` (App Group notifications)
//!
//! `WarrenTunnelParameters` mirrors `warren-tunnel::WarrenTunnelParameters` and
//! includes: exit pubkey, exit IP:port, wallet signing key, optional multi-hop
//! relay list, optional DAITA spec, optional NAT-PMP enabled, optional
//! bypass_cidrs (cf. M4.H.G `--bypass-cidr`).
//!
//! Underlying crates (path-deps to add when wiring):
//! - `warren-tunnel` (Quinn connection, multi-hop pump, DAITA)
//! - `warren-backoff` (reconnect 15s `Backoff::HANDSHAKE`, cf. M4.H.G)
//!
//! iOS specifics:
//! - Uses `NEPacketTunnelFlow.readPackets`/`writePackets` for IP packet I/O.
//! - The `packet_fd: i32` parameter is the file descriptor exposed by
//!   NetworkExtension (cf. `tun_rs` macOS backend, but iOS uses
//!   `utun` directly via NetworkExtension framework, so the integration
//!   path may need `PacketDevice::from_fd(OwnedFd)` cross-repo — same
//!   blocker as Session D Android (`tun_rs 2.8` lacks Android backend).
