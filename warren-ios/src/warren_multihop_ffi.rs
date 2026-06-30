//! Warren multi-hop FFI for iOS (HPKE handshake via warrenguard-multihop).
//!
//! Skeleton: implementations land with the PacketTunnelProvider
//! multi-hop wiring + UI multi-hop view. The Swift side surfaces a
//! `WarrenMultiHopConfig` that the user configures from settings
//! (entry country + exit country pickers).
//!
//! Intended exports:
//! - `warren_multihop_init_handshake(entry_pubkey: *const u8, exit_pubkey: *const u8, out_session: *mut WarrenMultiHopSession)`
//! - `warren_multihop_encrypt_payload(session, payload, len, out_encrypted)`
//! - `warren_multihop_destroy_session(session: *mut WarrenMultiHopSession)`
//!
//! Underlying crates (path-deps to add when wiring):
//! - `warrenguard-multihop` (HPKE handshake, pre-HPKE padding marker `0xFF`
//!   from MultiHopClient)
//! - `warren-relay-selector` (country picker, exposing `WarrenRelayList`
//!   for the UI)
//!
//! Note: multi-hop is OFF by default in the UI. The exported FFI must therefore accept
//! a "single-hop" mode where only the exit relay is used and the
//! multi-hop handshake is bypassed.
