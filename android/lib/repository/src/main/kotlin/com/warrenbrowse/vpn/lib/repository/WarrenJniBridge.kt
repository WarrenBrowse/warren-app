package com.warrenbrowse.vpn.lib.repository

/**
 * Lib-side abstraction over the Warren JNI surface consumed by
 * repository-layer code. Lets `lib/repository` call into native code
 * without depending on the concrete `com.warrenbrowse.vpn.jni.WarrenJni`
 * object that lives in the `:app` module (which would create a
 * dependency arrow from a `lib/<x>` module to the application module,
 * forbidden in this codebase's layering).
 *
 * The concrete implementation lives in `app/jni/WarrenJniBridgeImpl`
 * and is bound to this interface via Koin (`AppModule`). Tests can
 * substitute a fake implementation that does not require the
 * `libwarren_jni.so` native library.
 *
 * Method surface is intentionally narrow: only the JNI exports that
 * `lib/repository` actually invokes are listed. Lifecycle-related
 * exports (initLogger, connectTunnel, disconnectTunnel, ...) stay
 * private to the `:app` module that owns the tunnel session.
 *
 * Note: KDoc comments must avoid the `/<asterisk>` substring (i.e. a
 * literal slash directly followed by an asterisk like a glob path)
 * because Kotlin's nested-block-comment parser treats it as an inner
 * comment opener and falls off the end of the file looking for a
 * matching close. Use `<x>` or `(any submodule)` for variable paths.
 */
interface WarrenJniBridge {
    /** Generate a fresh 12-word BIP39 English mnemonic. */
    fun generateMnemonic(): String

    /**
     * Derive the wallet identity as a Warren SS58 address (a `wb…`
     * string, 47-49 chars) from a BIP39 mnemonic phrase.
     */
    fun mnemonicPubkeySs58(mnemonic: String): String


    /**
     * Both verdicts of the signed `android.json` update manifest
     * (Ed25519-verified) from one fetch: whether [currentVersion] may keep
     * running, and the newest stable version newer than it. Any error answers
     * [WarrenVersionVerdict.UNKNOWN] (fail-open on support, fail-closed on the
     * prompt). Blocks on a network fetch, so callers must invoke it off the
     * main thread.
     */
    fun fetchVersionInfo(currentVersion: String): WarrenVersionVerdict

    /**
     * Fetch the public `GET /v1/network` environment descriptor as a
     * JSON string (`{"ok":true,...}` or `{"ok":false}` on any failure).
     * Unauthenticated display data. Blocks on a network fetch, so
     * callers must invoke it off the main thread.
     */
    fun fetchNetworkInfo(): String

    /**
     * Sign and submit the community-forum login for [sid] against the
     * allowlisted connect [host] in Rust (`POST /v1/forum/login`); the wallet
     * signature never surfaces here. Returns the JSON envelope of
     * `warren_forum::envelope`. Blocks on a network POST: invoke off the main
     * thread. Never log the mnemonic or the sid.
     */
    fun forumLogin(mnemonic: String, sid: String, host: String): String

    /**
     * Best-effort notify the connect [host] that the user declined the login
     * for [sid], so the waiting browser page unblocks. Unsigned; failures are
     * ignored. Blocks on a network POST: invoke off the main thread.
     */
    fun forumLoginCancel(sid: String, host: String)

    /**
     * Sign and submit an in-app bug report (`POST /v1/forum/report`) in Rust.
     * [reportJson] is one JSON object with the connect contract's field names;
     * [logGz] the gzipped redacted problem report, or null to file the report
     * without logs. Returns the JSON envelope of `warren_forum::report_envelope`.
     * Blocks on a network POST: invoke off the main thread.
     */
    fun forumReport(mnemonic: String, reportJson: String, logGz: ByteArray?): String

    /**
     * One conditional fetch of the broadcast forum activity digest, verified
     * in Rust against the pinned server key. Returns
     * `{"counts":"<hex>"|null,"fetch":"ok"|"not-modified"|"rejected"|"transport"}`:
     * `counts` is the whole anonymous document while a fresh one is held (one
     * lowercase hex character per slot), null otherwise; `fetch` sizes the
     * next delay. Carries no account. Blocks on a network GET: invoke off the
     * main thread.
     */
    fun forumDigestFetch(): String

    /**
     * The wallet's own forum notifications (`POST /v1/forum/notifications`),
     * signed and sent in Rust; the rows come back validated. Returns
     * `{"ok":true,"notifications":[..]}` or `{"ok":false,"error":..,"reason":..}`.
     * The one request tied to an account: call it only when the user opens
     * the panel. Blocks on a network POST: invoke off the main thread.
     */
    fun forumNotifications(mnemonic: String): String

    /**
     * Marks the wallet's forum notification list seen
     * (`POST /v1/forum/notifications/seen`), signed over its own path.
     * Returns `{"ok":true}` or the classed failure. Blocks on a network POST:
     * invoke off the main thread.
     */
    fun forumNotificationsSeen(mnemonic: String): String

    /**
     * Collect the redacted problem report into [outputPath]: the Rust log
     * files, the Kotlin log directory [appLogDir], a logcat dump, and the
     * [metadataJson] facts (one JSON object) in the header, with every string
     * of [redactJson] (a JSON array) redacted on top of the collector's own
     * rules. [forSend] says the report is about to be sent: only then do the
     * live network probes run (one of them on a socket that bypasses the
     * TUN); a report collected to be read reaches no host and records the
     * probe keys as `not-run`. Returns `{"ok":true,"bytes":N}` or
     * `{"ok":false,"error":..}`. Reads files and runs `logcat`: invoke off
     * the main thread.
     */
    fun collectProblemReport(
        metadataJson: String,
        redactJson: String,
        appLogDir: String,
        outputPath: String,
        forSend: Boolean,
    ): String
}

/**
 * What the update manifest says about the running version: [isSupported] drives
 * the forced-update gate, [latestAvailable] the "update available" prompt
 * (`null` when no newer stable release is known).
 */
data class WarrenVersionVerdict(val isSupported: Boolean, val latestAvailable: String?) {
    companion object {
        /** The answer when the manifest could not be read. */
        val UNKNOWN = WarrenVersionVerdict(isSupported = true, latestAvailable = null)
    }
}
