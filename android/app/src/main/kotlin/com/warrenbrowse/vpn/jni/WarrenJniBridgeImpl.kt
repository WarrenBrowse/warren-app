package com.warrenbrowse.vpn.jni

import com.warrenbrowse.vpn.lib.repository.WarrenJniBridge
import com.warrenbrowse.vpn.lib.repository.WarrenVersionVerdict
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive

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


    override fun fetchVersionInfo(currentVersion: String): WarrenVersionVerdict =
        try {
            val obj = Json.parseToJsonElement(WarrenJni.fetchVersionInfo(currentVersion)).jsonObject
            WarrenVersionVerdict(
                isSupported = obj["supported"]?.jsonPrimitive?.booleanOrNull ?: true,
                latestAvailable = obj["latest"]?.jsonPrimitive?.contentOrNull?.ifEmpty { null },
            )
        } catch (e: IllegalArgumentException) {
            // A malformed envelope reads as "nothing known", the same fail-open
            // and fail-closed pair the native side answers on its own failures.
            WarrenVersionVerdict.UNKNOWN
        }

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
        forSend: Boolean,
    ): String = WarrenJni.collectProblemReport(metadataJson, redactJson, appLogDir, outputPath, forSend)
}
