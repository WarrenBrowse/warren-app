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

    // -- BIP39 mnemonic + Ed25519 wallet -----------------------------------
    //
    // Backed by warren-identity (`warren-jni/src/wallet.rs`). The three
    // primitives are stateless: the mnemonic is passed in per signing call
    // rather than cached in JNI memory, so the secret never lives in the
    // Rust process beyond the call boundary. Kotlin owns the
    // Keystore-encrypted persistence (D.5 wallet feature module).

    /** Generate a fresh 12-word BIP39 English mnemonic. */
    external fun generateMnemonic(): String

    /**
     * Parse `mnemonic` and return the derived Ed25519 public key (32 raw
     * bytes). Throws a Java exception if the mnemonic is malformed.
     */
    external fun importMnemonic(mnemonic: String): ByteArray

    /**
     * Convenience wrapper: return the wallet pubkey directly as a 64-char
     * lowercase hex string, ready for the `X-Warren-Pubkey-Hex` header.
     * Saves Kotlin from a bytes-to-hex round-trip when only the hex form is
     * needed.
     */
    external fun mnemonicPubkeyHex(mnemonic: String): String

    /**
     * Ed25519-sign `canonicalMessage` with the key derived from `mnemonic`.
     * Returns a 64-byte signature suitable for the `X-Warren-Signature`
     * header (cf. warren-identity `auth::canonical_message`).
     *
     * Prefer [signCanonicalRequest] for actual API authentication: it
     * builds the canonical byte format from the 5 `X-Warren-*` fields in
     * Rust, keeping wire-format ownership single-source.
     */
    external fun signRequest(mnemonic: String, canonicalMessage: ByteArray): ByteArray

    /**
     * Build the canonical message from the 5 `X-Warren-*` request fields
     * and Ed25519-sign it with the key derived from `mnemonic`. Returns a
     * 64-byte signature.
     *
     * @param method HTTP verb, uppercase (`"GET"`, `"POST"`, ...)
     * @param path URL path with leading `/` (no host)
     * @param timestamp Unix epoch seconds (rejected if negative)
     * @param nonceHex 16-byte random nonce, hex-encoded (no `0x`)
     * @param bodyHashHex SHA-256 of the request body, hex-encoded
     *   (empty-string SHA-256 for GET / empty body)
     */
    external fun signCanonicalRequest(
        mnemonic: String,
        method: String,
        path: String,
        timestamp: Long,
        nonceHex: String,
        bodyHashHex: String,
    ): ByteArray

    // -- Tunnel lifecycle (D.4) --------------------------------------------

    /**
     * Start a Warren Quinn tunnel on the supplied TUN file descriptor.
     *
     * @param tunFd raw fd duplicated from `VpnService.Builder.establish()`
     * @param mnemonic BIP39 mnemonic used to derive the wallet `SigningKey`
     *  that authenticates the QUIC handshake. Passed per-call so the secret
     *  never lives in JNI memory beyond the spawned session task.
     * @param configJson serde-encoded `WarrenTunnelConfig` (exit pubkey,
     *  optional multi-hop entry, optional DAITA spec, bypass CIDRs,
     *  NAT-PMP toggle, wallet pubkey).
     * @return 0 on success, negative on synchronous error (exception
     *  also thrown). Long-running connect + handshake happens
     *  asynchronously; poll [getTunnelStatus] for transitions.
     */
    external fun connectTunnel(tunFd: Int, mnemonic: String, configJson: String): Int

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

    /**
     * Returns a JSON-encoded array of relay descriptors. Each entry is a
     * `RelayInfo`-shaped object with fields:
     *   - `exit_id`        : 16-byte stable identifier, lowercase hex
     *   - `exit_pubkey_hex`: 32-byte Ed25519 pubkey, lowercase hex
     *   - `endpoint`       : "host:port" UDP endpoint
     *   - `country`        : ISO 3166-1 alpha-2
     *   - `city`           : city name
     *   - `active`         : whether the relay accepts new sessions today
     *   - `weight`         : selector-weight hint (higher = preferred)
     *
     * D.6 wired: fetches `GET /v1/exits` via warren-api-client and
     * verifies the embedded server signature. On network / signature
     * failure, falls back to a hardcoded warren-exit-1 entry so the
     * picker UI is never empty (a `WarrenJni` warning is logged).
     */
    external fun listRelays(): String

    // -- Problem report (D.6) ----------------------------------------------

    /**
     * Submit a problem report bundle to the operator's support inbox.
     * Returns a JSON object `{"ok": Boolean, "reference_id": String?,
     * "error": String?}`. `reference_id` is present iff `ok` is true.
     *
     * @param mnemonic BIP39 mnemonic used to sign the request with the
     *  user's wallet pubkey (= the user identity on the server side).
     * @param userMessage free-form user description (max 4 KiB).
     * @param redactedLogs newline-joined redacted log lines (max 256 KiB).
     *  Pass an empty string when the user unchecked "Include logs".
     * @param appVersion app version string ("1.2.3"), free-form.
     * @param platform platform tag ("android-arm64"), free-form.
     */
    external fun sendProblemReport(
        mnemonic: String,
        userMessage: String,
        redactedLogs: String,
        appVersion: String,
        platform: String,
    ): String

    /**
     * Collect the latest redacted log bundle from the Rust ring buffer.
     * Returns an empty string until the file-appender wiring lands; the
     * Kotlin side should treat the empty case as "no logs available"
     * and ship the report with `redactedLogs = ""`.
     */
    external fun collectReport(): String
}
