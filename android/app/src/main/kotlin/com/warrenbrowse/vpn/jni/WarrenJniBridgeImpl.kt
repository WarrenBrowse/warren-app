package com.warrenbrowse.vpn.jni

import com.warrenbrowse.vpn.lib.repository.WarrenJniBridge

/**
 * Default [WarrenJniBridge] implementation: delegates each method to
 * the global [WarrenJni] facade. Kept in the `:app` module because
 * [WarrenJni] (which holds the `System.loadLibrary("warren_jni")`
 * static-init block) lives here too.
 *
 * Lifecycle-coupled exports of [WarrenJni] (initLogger, connectTunnel,
 * etc.) intentionally do NOT appear here; they stay app-private.
 */
class WarrenJniBridgeImpl : WarrenJniBridge {
    override fun generateMnemonic(): String = WarrenJni.generateMnemonic()

    override fun mnemonicPubkeySs58(mnemonic: String): String =
        WarrenJni.mnemonicPubkeySs58(mnemonic)

    override fun collectReport(): String = WarrenJni.collectReport()
}
