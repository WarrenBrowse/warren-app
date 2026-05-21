package com.warrenbrowse.vpn.jni

// Kotlin facade over the `warren-jni` Rust library.
//
// All external functions resolve to symbols of the form
// `Java_com_warrenbrowse_vpn_jni_WarrenJni_<name>` exported by
// `libwarren_jni.so` (cf. `warren-jni/src/lib.rs`).
//
// D.3 scope: declarations + library load. The actual native implementations
// are best-effort stubs - real wiring against warren-core lands in D.4 (tunnel
// lifecycle) and D.5 (mnemonic + signing). See
// `.planning/session-d-d3-warren-jni-design.md` for the migration plan.
object WarrenJni {
    init {
        System.loadLibrary("warren_jni")
    }

    /**
     * Initialise the Rust-side logger + shared tokio runtime. Must be called
     * once during process startup (typically from `WarrenApplication.onCreate`).
     */
    external fun initLogger(filesDirectory: String)

    // -- BIP39 mnemonic + Ed25519 wallet (D.5) -----------------------------

    /** Generate a fresh 12-word BIP39 English mnemonic. */
    external fun generateMnemonic(): String

    /**
     * Import an existing BIP39 mnemonic and return the derived Ed25519 public
     * key (32 raw bytes).
     */
    external fun importMnemonic(mnemonic: String): ByteArray

    /**
     * Sign canonical request bytes with the active wallet's Ed25519 key.
     * Returns a 64-byte signature suitable for the `X-Warren-Signature` header.
     */
    external fun signRequest(canonicalMessage: ByteArray): ByteArray

    // -- Tunnel lifecycle (D.4) --------------------------------------------

    /**
     * Start a Warren Quinn tunnel on the supplied TUN file descriptor.
     *
     * @param tunFd raw fd duplicated from `VpnService.Builder.establish()`
     * @param configJson serde-encoded `WarrenTunnelConfig` (exit pubkey,
     *  optional multi-hop entry, optional DAITA spec, bypass CIDRs,
     *  NAT-PMP toggle, wallet pubkey).
     * @return 0 on success, negative on error (exception also thrown).
     */
    external fun connectTunnel(tunFd: Int, configJson: String): Int

    /** Stop the active tunnel. No-op if none is running. */
    external fun disconnectTunnel()

    /**
     * Returns the current tunnel state:
     * - 0: disconnected
     * - 1: connecting
     * - 2: connected
     * - 3: reconnecting
     */
    external fun getTunnelStatus(): Int
}
