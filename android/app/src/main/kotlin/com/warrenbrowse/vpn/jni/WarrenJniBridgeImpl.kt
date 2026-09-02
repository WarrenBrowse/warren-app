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


    override fun checkVersionSupported(currentVersion: String): Boolean =
        WarrenJni.checkVersionSupported(currentVersion) == 1

    override fun latestAvailableVersion(currentVersion: String): String? =
        WarrenJni.latestAvailableVersion(currentVersion).ifEmpty { null }

    override fun fetchNetworkInfo(): String = WarrenJni.fetchNetworkInfo()

    override fun forumLogin(mnemonic: String, sid: String, host: String): String =
        WarrenJni.forumLogin(mnemonic, sid, host)

    override fun forumLoginCancel(sid: String, host: String) = WarrenJni.forumLoginCancel(sid, host)

    override fun forumReport(mnemonic: String, reportJson: String, logGz: ByteArray?): String =
        WarrenJni.forumReport(mnemonic, reportJson, logGz)

    override fun collectProblemReport(
        metadataJson: String,
        redactJson: String,
        appLogDir: String,
        outputPath: String,
    ): String = WarrenJni.collectProblemReport(metadataJson, redactJson, appLogDir, outputPath)
}
