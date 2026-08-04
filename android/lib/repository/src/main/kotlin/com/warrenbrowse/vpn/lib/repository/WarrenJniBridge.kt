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
     * Return whether the running [currentVersion] is still supported, based on
     * the signed `android.json` update manifest (Ed25519-verified). `true` =
     * supported, `false` = must force-update. Fail-open on any error. Blocks on
     * a network fetch, so callers must invoke it off the main thread.
     */
    fun checkVersionSupported(currentVersion: String): Boolean

    /**
     * Return the newest stable version newer than [currentVersion], or `null`
     * when there is none / on any error, based on the same signed `android.json`
     * manifest (Ed25519-verified). Fail-closed: a false "update available" is
     * never reported. Blocks on a network fetch, so callers must invoke it off
     * the main thread.
     */
    fun latestAvailableVersion(currentVersion: String): String?

    /**
     * Fetch the public `GET /v1/network` environment descriptor as a
     * JSON string (`{"ok":true,...}` or `{"ok":false}` on any failure).
     * Unauthenticated display data. Blocks on a network fetch, so
     * callers must invoke it off the main thread.
     */
    fun fetchNetworkInfo(): String
}
