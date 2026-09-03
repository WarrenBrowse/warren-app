//! The one place the IPC layer's decoding cap and the forum wire contract are
//! visible at the same time.
//!
//! `mullvad-management-interface` is the IPC layer and does not depend on the
//! forum crate, so it spells the gzipped-log cap out; that copy is held equal
//! to the forum crate's own constant here, the way the client-rule fixtures
//! hold the platform tables to the Rust one. Without this, the broker could
//! raise its log cap and the daemon would keep refusing an at-cap report at
//! the socket, before any handler ran.

use mullvad_management_interface::{MAX_RPC_MESSAGE_BYTES, MAX_RPC_MESSAGE_OVERHEAD_BYTES};
use warren_forum::MAX_LOG_GZ_BYTES;

#[test]
fn the_management_decoding_cap_is_the_forum_log_cap_plus_the_declared_headroom() {
    assert_eq!(
        MAX_RPC_MESSAGE_BYTES,
        MAX_LOG_GZ_BYTES + MAX_RPC_MESSAGE_OVERHEAD_BYTES,
        "the IPC cap drifted from warren-forum's own gzipped-log cap"
    );
}
