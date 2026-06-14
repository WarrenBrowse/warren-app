package com.warrenbrowse.vpn.jni

import android.net.VpnService

// Kotlin facade over the `warren-jni` Rust library.
//
// All external functions resolve to symbols of the form
// `Java_com_warrenbrowse_vpn_jni_WarrenJni_<name>` exported by
// `libwarren_jni.so` (cf. `warren-jni/src/lib.rs`).
//
// See `.planning/session-d-d3-warren-jni-design.md` for the migration plan.
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
     * Convenience wrapper: return the wallet identity directly as a Warren
     * SS58 address (a `wb…` string, 47-49 chars), ready for the
     * `X-Warren-PubKey` header. Saves Kotlin from a bytes-to-address
     * round-trip when only the SS58 form is needed.
     */
    external fun mnemonicPubkeySs58(mnemonic: String): String

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

    // -- Tunnel lifecycle --------------------------------------------------

    /**
     * Start a Warren Quinn tunnel on the supplied TUN file descriptor.
     *
     * @param vpnService the owning `VpnService`. Its `protect(int)` is
     *  called on the tunnel's own UDP socket so that socket bypasses the
     *  VPN routes (binds to the underlying physical network). Without it
     *  the handshake packets loop back into the TUN and never reach the
     *  exit.
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
    external fun connectTunnel(
        vpnService: VpnService,
        tunFd: Int,
        mnemonic: String,
        configJson: String,
    ): Int

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
     * Returns the live NAT-PMP port-forwarding status as a JSON object:
     *   `{"state": "idle"|"requesting"|"mapped"|"rate_limited"|"failed",
     *     "external_port": Int?, "lifetime_secs": Int?,
     *     "retry_after_secs": Int?, "reason": String?}`
     *
     * Polled alongside [getTunnelStatus]. `idle` when port forwarding is
     * off or no mapping is active.
     */
    external fun getNatPmpStatus(): String

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

    /**
     * Fetch the wallet's subscription status via a signed
     * `GET /v1/subscription`. The [mnemonic] derives the signing key at
     * the JNI boundary and is not retained.
     *
     * Returns a JSON object:
     *   - `{"ok": true, "expires_at": <unix seconds>}` on success
     *   - `{"ok": false, "error": "..."}` on failure
     */
    external fun getSubscription(mnemonic: String): String

    /**
     * Redeem a subscription voucher (`POST /v1/register`), binding the
     * wallet pubkey to a new subscription. Returns
     * `{"ok": true, "expires_at": <unix seconds>}` or
     * `{"ok": false, "error": "..."}`. The voucher and mnemonic are not
     * retained.
     */
    external fun redeemVoucher(mnemonic: String, voucher: String): String

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

    /**
     * Return whether the running app version is still supported (allowed to
     * keep running). `1` = supported, `0` = must force-update.
     *
     * Fetches the signed `android.json` update manifest from the
     * Let's-Encrypt-pinned host, verifies its Ed25519 signature against the
     * embedded trusted pubkey, then applies the shared
     * `minimum_supported_version` rule (same verifier as the desktop app).
     *
     * Fail-open: any transient failure (network, signature, unparseable
     * version, runtime not ready) returns `1` so a flaky network never locks
     * the user out. Blocks the calling thread on a network fetch, so it must
     * be invoked off the main thread.
     *
     * @param currentVersion the running app version string (`BuildConfig.VERSION_NAME`).
     */
    external fun checkVersionSupported(currentVersion: String): Int
}
