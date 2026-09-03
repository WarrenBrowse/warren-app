package com.warrenbrowse.vpn.jni

import android.net.VpnService

// Kotlin facade over the `warren-jni` Rust library.
//
// All external functions resolve to symbols of the form
// `Java_com_warrenbrowse_vpn_jni_WarrenJni_<name>` exported by
// `libwarren_jni.so` (cf. `warren-jni/src/lib.rs`).
object WarrenJni {
    init {
        System.loadLibrary("warren_jni")
    }

    /**
     * Initialise the Rust-side logger + shared tokio runtime. Must be called once during process
     * startup (typically from `WarrenApplication.onCreate`).
     */
    external fun initLogger(filesDirectory: String)

    /**
     * The per-environment product anchors compiled into the native library (the Rust
     * `warren-product-env` crate) as one JSON object, decoded by
     * [com.warrenbrowse.vpn.app.product.ProductAnchors]: the scheme, the application id, the API
     * host, the connect host and the forum origin of the environment `libwarren_jni.so` was built
     * for. Pure: needs neither [initLogger] nor the runtime.
     */
    external fun productAnchorsJson(): String

    // -- BIP39 mnemonic + Ed25519 wallet -----------------------------------
    //
    // Backed by warren-identity (`warren-jni/src/wallet.rs`). The three
    // primitives are stateless: the mnemonic is passed in per signing call
    // rather than cached in JNI memory, so the secret never lives in the
    // Rust process beyond the call boundary. Kotlin owns the
    // Keystore-encrypted persistence (wallet feature module).

    /** Generate a fresh 12-word BIP39 English mnemonic. */
    external fun generateMnemonic(): String

    /**
     * Parse `mnemonic` and return the derived Ed25519 public key (32 raw bytes). Throws a Java
     * exception if the mnemonic is malformed.
     */
    external fun importMnemonic(mnemonic: String): ByteArray

    /**
     * Convenience wrapper: return the wallet identity directly as a Warren SS58 address (a `wb…`
     * string, 47-49 chars), ready for the `X-Warren-PubKey` header. Saves Kotlin from a
     * bytes-to-address round-trip when only the SS58 form is needed.
     */
    external fun mnemonicPubkeySs58(mnemonic: String): String

    /**
     * Ed25519-sign `canonicalMessage` with the key derived from `mnemonic`. Returns a 64-byte
     * signature suitable for the `X-Warren-Signature` header (cf. warren-identity
     * `auth::canonical_message`).
     *
     * Prefer [signCanonicalRequest] for actual API authentication: it builds the canonical byte
     * format from the 5 `X-Warren-*` fields in Rust, keeping wire-format ownership single-source.
     */
    external fun signRequest(mnemonic: String, canonicalMessage: ByteArray): ByteArray

    /**
     * Build the canonical message from the 5 `X-Warren-*` request fields and Ed25519-sign it with
     * the key derived from `mnemonic`. Returns a 64-byte signature.
     *
     * @param method HTTP verb, uppercase (`"GET"`, `"POST"`, ...)
     * @param path URL path with leading `/` (no host)
     * @param timestamp Unix epoch seconds (rejected if negative)
     * @param nonceHex 16-byte random nonce, hex-encoded (no `0x`)
     * @param bodyHashHex SHA-256 of the request body, hex-encoded (empty-string SHA-256 for GET /
     *   empty body)
     */
    external fun signCanonicalRequest(
        mnemonic: String,
        method: String,
        path: String,
        timestamp: Long,
        nonceHex: String,
        bodyHashHex: String,
    ): ByteArray

    /**
     * Sign and submit a forum-login challenge for `sid` to the connect `host` (`POST
     * /v1/forum/login`), mirroring the desktop daemon's SignForumLogin plus its POST. Everything
     * wire-sensitive happens in Rust: the canonical body `{"sid":"<sid>"}`, the four `X-Warren-*`
     * headers, a fresh timestamp and random nonce, and the HTTPS POST itself. Only `sid` and `host`
     * cross the boundary, so the wallet signature never surfaces to Kotlin. `host` is checked
     * against a hard allowlist in Rust so a hostile deep link cannot redirect the signed request.
     *
     * Before signing, Rust reads the session's status once: the `Date` of that answer corrects a
     * device clock the server would otherwise refuse, and a session already gone is reported
     * without spending a signature.
     *
     * Returns a JSON string (`warren_forum::envelope`):
     * - `{"ok":true}` when the provider accepted the login, with `handle` and `notify_slot` when
     *   the body carried the forum identity
     * - `{"ok":false,"error":"subscription-required"}` when the wallet has never subscribed
     *   (HTTP 403)
     * - `{"ok":false,"error":"clock-skew"}` when the provider refused the signature for a device
     *   clock outside its window
     * - `{"ok":false,"error":"expired"}` when the session is gone
     * - `{"ok":false,"error":"error","reason":"<class>"}` on any other failure (`runtime`, `build`,
     *   `transport`, `http-<status>`)
     *
     * Blocks on a network POST, so it must be invoked off the main thread. Never log the mnemonic
     * or the sid (no-log).
     *
     * @param sid 32 lowercase hex chars (validated in Rust; rejected otherwise)
     * @param host connect host from the deep link (allowlisted in Rust)
     */
    external fun forumLogin(mnemonic: String, sid: String, host: String): String

    /**
     * Best-effort notify the connect `host` that the user declined the forum login for `sid` (`POST
     * /v1/session/<sid>/cancel`), so the waiting browser page unblocks instead of polling to
     * timeout. Unsigned (no wallet material); failures are ignored (the server session expires in 5
     * min). Blocks on a network POST, so it must be invoked off the main thread.
     */
    external fun forumLoginCancel(sid: String, host: String)

    /**
     * Sign and submit an in-app bug report (`POST /v1/forum/report`) in Rust: `reportJson` is one
     * JSON object with the connect contract's field names, `logGz` the gzipped redacted problem
     * report or null. The body is serialised once in Rust, signed, and sent; only the form and the
     * gzip cross the boundary. Returns the envelope of `warren_forum::report_envelope`
     * (`{"ok":true,"topic_id":..,"topic_url":.., "logs":..,"handle":..,"notify_slot":..}` or
     * `{"ok":false,"error":..}`). Blocks on a network POST, so it must be invoked off the main
     * thread.
     */
    external fun forumReport(mnemonic: String, reportJson: String, logGz: ByteArray?): String

    /**
     * One conditional fetch of the broadcast forum activity digest (`GET /v1/forum/digest` on the
     * API host), verified in Rust against the pinned server key with the daemon's anti-rollback and
     * freshness rules. Returns `{"counts":"<hex>"|null,"fetch":"ok"|"not-modified"|"rejected"|
     * "transport"}`: `counts` is the whole anonymous document while a fresh one is held, `fetch`
     * sizes the next delay. The document is identical for every client, so the request carries no
     * account. Blocks on a network GET: invoke off the main thread.
     */
    external fun forumDigestFetch(): String

    /**
     * One fetch of the operator broadcast notices (`GET /v1/notices` on the API host), verified in
     * Rust against the pinned server key with the daemon's anti-rollback and freshness rules.
     * Returns `{"notices":[{"id":..,"message":..,"level":"info"|"warning"|"error"}],
     * "fetch":"ok"|"rejected"|"transport"}`: the list is what the banner must show right now,
     * already filtered for the signed envelope expiry, each notice's own TTL and [currentVersion]
     * (`BuildConfig.VERSION_NAME`). The route is public and unauthenticated, and the request
     * carries nothing about the caller. Blocks on a network GET: invoke off the main thread.
     */
    external fun noticesFetch(currentVersion: String): String

    /**
     * One fetch of the launch announcements (`GET /v1/announcements` on the API host), verified in
     * Rust against the pinned server key with the daemon's anti-rollback and freshness rules.
     * Returns `{"announcements":[{"id":..,"headline":..,"body":..,"level":..,
     * "cta":{"label":..,"url":..}|null,"voucher_campaign_id":..|null}],
     * "fetch":"ok"|"rejected"|"transport"}`: the list is what the card must show right now, already
     * filtered for the signed envelope expiry, each announcement's own TTL and [currentVersion]
     * (`BuildConfig.VERSION_NAME`), with a call to action whose URL was refused already withheld.
     * The route is public and unauthenticated, the document is byte-identical for every caller, and
     * the request carries nothing about the account. Blocks on a network GET: invoke off the main
     * thread.
     */
    external fun announcementsFetch(currentVersion: String): String

    /**
     * This account's code for the campaign an announcement offers
     * (`GET /v1/campaign/{campaignId}/voucher`), signed with the wallet key and sent in Rust.
     * Returns `{"ok":true,"code":".."}`, `{"ok":true,"code":null}` when the account is outside the
     * cohort, or `{"ok":false,"code":null}` when the lookup failed. A per-account value cannot ride
     * the broadcast document, which is byte-identical for every caller; this is the one request the
     * offer makes that is tied to an account, and it is made only when an announcement carries a
     * campaign. Blocks on a network GET: invoke off the main thread. Never log the mnemonic, and
     * never log the code: it is a bearer token worth a month of service.
     */
    external fun campaignVoucher(mnemonic: String, campaignId: String): String

    /**
     * The wallet's own forum notifications (`POST /v1/forum/notifications`), signed with the wallet
     * key and sent in Rust; the rows are validated there (kinds, pinned paths, bounded text) and
     * come back as `{"ok":true,"notifications":[{id,kind,unread,created_at,title,actor,excerpt,
     * path}]}` or `{"ok":false,"error":"error","reason":"<class>"}`. Blocks on a network POST:
     * invoke off the main thread. Never log the mnemonic.
     */
    external fun forumNotifications(mnemonic: String): String

    /**
     * Marks the wallet's forum notification list seen (`POST /v1/forum/notifications/seen`), what
     * opening the forum bell does there; signed over its own path so the read's signature cannot
     * be replayed as this write. Returns `{"ok":true}` or the classed failure. Blocks on a network
     * POST: invoke off the main thread.
     */
    external fun forumNotificationsSeen(mnemonic: String): String

    /**
     * Report an exit this client gave up on (`POST /v1/incidents/exit-down`), so an outage only
     * this client can see reaches the operator's exit-health feed. Signed with the wallet key in
     * Rust; the server records the exit and the count, never the signer. Fire and forget, and
     * budgeted there (three back to back, then one per 20 minutes) so a client stuck in a failover
     * loop cannot drown the feed. Returns `{"ok":true}` or
     * `{"ok":false,"reason":"<class>"}`; nothing about the connection depends on the answer.
     * Blocks on a network POST: invoke off the main thread. Never log the mnemonic.
     */
    external fun reportExitDown(mnemonic: String, exitPubkeyHex: String): String

    /**
     * Report a pinned exit key that changed (`POST /v1/incidents/pubkey-mismatch`), what the
     * "Report to Warren" choice on the key-mismatch dialog sends. Every field is already public
     * through the signed relay list and the server records no signer, so the report says what
     * changed and not who saw it. Returns `{"ok":true}` or `{"ok":false,"reason":"<class>"}`.
     * Blocks on a network POST: invoke off the main thread. Never log the mnemonic.
     */
    external fun reportPubkeyMismatch(
        mnemonic: String,
        exitIdHex: String,
        oldPubkeyHex: String,
        newPubkeyHex: String,
        countryCode: String,
        city: String,
    ): String

    /**
     * Collect the redacted problem report into `outputPath` with the shared desktop collector: Rust
     * log files, the Kotlin log directory, a logcat dump, plus the `metadataJson` facts in the
     * header and the strings of `redactJson` redacted. `forSend` runs the live network probes,
     * which a report collected only to be read must not (`WarrenJniBridge`). Returns
     * `{"ok":true,"bytes":N}` or `{"ok":false,"error":..}`. Reads files and runs `logcat`: invoke
     * off the main thread.
     */
    external fun collectProblemReport(
        metadataJson: String,
        redactJson: String,
        appLogDir: String,
        outputPath: String,
        forSend: Boolean,
    ): String

    // -- Tunnel lifecycle --------------------------------------------------

    /**
     * Start a Warren Quinn tunnel on the supplied TUN file descriptor.
     *
     * @param vpnService the owning `VpnService`. Its `protect(int)` is called on the tunnel's own
     *   UDP socket so that socket bypasses the VPN routes (binds to the underlying physical
     *   network). Without it the handshake packets loop back into the TUN and never reach the exit.
     * @param tunFd raw fd duplicated from `VpnService.Builder.establish()`
     * @param mnemonic BIP39 mnemonic used to derive the wallet `SigningKey` that authenticates the
     *   QUIC handshake. Passed per-call so the secret never lives in JNI memory beyond the spawned
     *   session task.
     * @param configJson serde-encoded `WarrenTunnelConfig` (exit pubkey, optional multi-hop entry,
     *   optional DAITA spec, bypass CIDRs, NAT-PMP toggle, wallet pubkey).
     * @return 0 on success, negative on synchronous error (exception also thrown). Long-running
     *   connect + handshake happens asynchronously; wait on [awaitStatusChange] and read
     *   [getTunnelStatus] for transitions.
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
     * Waits up to [timeoutMs] for the session [disconnectTunnel] wound down to actually finish. `1`
     * = gone (or nothing to wait for), `0` = timed out.
     *
     * Dialling the next session before this returns registers it on the exit while the previous one
     * still holds the sticky inner IP, and the exit then has two live claims on one downlink route.
     */
    external fun awaitTunnelClosed(timeoutMs: Int): Int

    /**
     * Report an underlying-network handover (Wi-Fi to cellular and back) to the native migration
     * watchdog. It rebinds the live QUIC endpoint onto a fresh `VpnService.protect`-ed socket and
     * revalidates the path in about one RTT, so the session survives the handover instead of being
     * torn down for a full re-handshake.
     *
     * Never blocks and never throws: it hands one event to a channel. When the path cannot be
     * recovered the watchdog ends the session, so [getTunnelStatus] reports disconnected and the
     * Kotlin fail-closed policy runs the handover fallback. A no-op when no tunnel is running.
     */
    external fun notifyNetworkChanged()

    /**
     * Blocks until the native status generation differs from [lastSeen] (a tunnel state edge, a
     * datapath verdict, a NAT-PMP transition or a landed redial), or [timeoutMs] elapsed, and
     * returns the generation now seen. Passing back the value received sleeps until the next
     * change; passing a stale one returns at once, so no change is ever missed between two reads.
     * Read [getTunnelStatus], [getPathHealth], [getNatPmpStatus] and [getAutoRecoveryCount] on
     * every return.
     *
     * Parks the calling thread on the engine runtime: call it from a thread that may block, never
     * from the main thread.
     */
    external fun awaitStatusChange(lastSeen: Long, timeoutMs: Int): Long

    /**
     * Returns the current tunnel state:
     * - 0: disconnected
     * - 1: connecting
     * - 2: connected
     * - 3: reconnecting (transparent native redial in flight; expected to land within seconds,
     *   escalated to 0 after 15 s of continuous loss, see `warren-jni/src/redial.rs`)
     * - 4: unauthorized (subscription lapsed / revoked; terminal)
     */
    external fun getTunnelStatus(): Int

    /**
     * Returns the number of automatic in-session recoveries (a native redial that landed after a
     * session loss) since process start. Monotonic; read on every [awaitStatusChange] wake and
     * summed with the Kotlin-side retry-loop accounting for the "Reconnections" row.
     */
    external fun getAutoRecoveryCount(): Int

    /**
     * Returns the engine's current datapath verdict: `0` healthy, `1` large frames dying (a
     * last-mile shrink, handled by the MSS/PMTU machinery), `2` both probe sizes dying, which is a
     * wedged datapath: the transport is up and nothing crosses it. Read on every
     * [awaitStatusChange] wake.
     */
    external fun getPathHealth(): Int

    /**
     * Returns the engine's "Reduced MTU" verdict: the usable inner payload in bytes once the live
     * path measured below the TUN MTU, `0` while it carries full-size packets. The desktop reads
     * the same post-connect sample off `connectedEndpoint.effectiveMtu`; it is about the path,
     * never about the MTU the user set. Read on every [awaitStatusChange] wake.
     */
    external fun getEffectiveMtu(): Int

    /**
     * Returns the live NAT-PMP port-forwarding status as a JSON object: `{"state":
     * "idle"|"requesting"|"mapped"|"rate_limited"|"failed", "external_port": Int?, "lifetime_secs":
     * Int?, "retry_after_secs": Int?, "reason": String?}`
     *
     * Read on every [awaitStatusChange] wake. `idle` when port forwarding is off or no mapping is
     * active.
     */
    external fun getNatPmpStatus(): String

    /**
     * Returns a JSON-encoded array of relay descriptors. Each entry is a `RelayInfo`-shaped object
     * with fields:
     * - `exit_id` : 16-byte stable identifier, lowercase hex
     * - `exit_pubkey_hex`: 32-byte Ed25519 pubkey, lowercase hex
     * - `endpoint` : "host:port" UDP endpoint
     * - `country` : ISO 3166-1 alpha-2
     * - `city` : city name
     * - `active` : whether the relay accepts new sessions today
     * - `weight` : selector-weight hint (higher = preferred)
     *
     * Fetches `GET /v1/exits` via warren-api-client and verifies the embedded server signature. On
     * network / signature failure, falls back to a hardcoded warren-exit-1 entry so the picker UI
     * is never empty (a `WarrenJni` warning is logged).
     */
    external fun listRelays(): String

    /**
     * The exit a location pin dials, chosen in Rust over the relay list. `pinJson` is the
     * `ExitPin` as `{"kind":"automatic"|"country"|"city"|"exit", ...}` and `relaysJson` the
     * [listRelays] array (the same schema, re-encoded from the snapshot the dial reads). Returns
     * `{"index":<position in relaysJson>}` or `{"index":null}` when the pin names nothing usable
     * (an empty scope, an inactive exit, an automatic pin, or an input Rust could not read).
     *
     * The choice is the shared `warren_discovery_core::pick_exit` rule the desktop daemon dials
     * by (highest weight, ties broken by the smallest exit id), so the same list resolves to the
     * same node on every platform. Pure: needs neither [initLogger] nor the runtime.
     */
    external fun resolveExitPin(pinJson: String, relaysJson: String): String

    /**
     * The exit an unpinned dial goes to, chosen in Rust: the shared pick among the active rows
     * of `exitCountry`, and among every active row when that country has none (a preference is
     * not a pin, so a country with nothing active still yields a circuit). An empty
     * `exitCountry` means none preferred. Same answer shape as [resolveExitPin]. Pure.
     */
    external fun resolveAutomaticExit(exitCountry: String, relaysJson: String): String

    /**
     * The exit a drop retry moves to, chosen in Rust: an active alternative inside `pinJson`'s
     * scope (an automatic pin narrowed to `exitCountry` when it is not empty), in the failed
     * exit's own country first, then anywhere the pin allows, never `failedExitPubkeyHex` itself.
     * Same JSON shapes and answer as [resolveExitPin]; `{"index":null}` when nothing fits. Pure.
     */
    external fun resolveFailoverExit(
        pinJson: String,
        exitCountry: String,
        relaysJson: String,
        failedExitPubkeyHex: String,
    ): String

    /**
     * Fetch the raw signed multi-hop directory (`GET /v1/multihop/directory`) as its verbatim JSON
     * string, or an empty string on any network failure.
     *
     * Called from the connect flow BEFORE the VpnService TUN is established, so the fetch egresses
     * the physical network (a fetch issued after the TUN is up would be captured by the half-open
     * tunnel and blackholed). The raw blob is handed to warren-jni `run_multi_hop_session` via the
     * tunnel config; the signature/version are verified Rust-side there, so passing the unverified
     * blob through the config carries no trust.
     */
    external fun fetchMultihopDirectory(): String

    /**
     * Fetch the wallet's subscription status via a signed `GET /v1/subscription`. The [mnemonic]
     * derives the signing key at the JNI boundary and is not retained.
     *
     * Returns a JSON object:
     * - `{"ok": true, "expires_at": <unix seconds>}` on success
     * - `{"ok": false, "error": "..."}` on failure
     */
    external fun getSubscription(mnemonic: String): String

    /**
     * Redeem a subscription voucher (`POST /v1/register`), binding the wallet pubkey to a new
     * subscription. Returns `{"ok": true, "expires_at": <unix seconds>}` or `{"ok": false, "error":
     * "..."}`. The voucher and mnemonic are not retained.
     */
    external fun redeemVoucher(mnemonic: String, voucher: String): String

    /**
     * Fetch the public `GET /v1/network` environment descriptor (unauthenticated display data:
     * environment label, degraded flag, default bandwidth cap, payments flag). Returns
     * `{"ok":true,"environment":...,"degraded":...,"default_rate_bps":..., "payments_enabled":...}`
     * or `{"ok":false}` on any failure. Blocks on a network fetch; call off the main thread.
     */
    external fun fetchNetworkInfo(): String

    /**
     * Both verdicts of the signed `android.json` update manifest from one fetch, as
     * `{"supported":bool,"latest":"x.y.z"}`: whether the running version may keep running (the
     * forced-update gate) and the newest stable release strictly newer than [currentVersion]
     * (`latest` empty when there is none). The manifest comes from the Let's-Encrypt-pinned host
     * and its Ed25519 signature is verified against the embedded trusted pubkey with the same
     * verifier as the desktop app.
     *
     * Fail-open on support and fail-closed on the prompt: any failure (network, signature,
     * unparseable version, runtime not ready) answers `{"supported":true,"latest":""}`, so a flaky
     * network never locks the user out and never shows an update that may not exist. Blocks on a
     * network fetch, so it must be invoked off the main thread.
     *
     * @param currentVersion the running app version string (`BuildConfig.VERSION_NAME`).
     */
    external fun fetchVersionInfo(currentVersion: String): String
}
