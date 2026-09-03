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
 * Every call first waits at [WarrenNativeRuntime]: the native init runs on
 * its own thread at process start, and these calls are all made off the main
 * thread, so the wait is a no-op once the init has run and the only way a
 * call is served before it otherwise.
 *
 * Lifecycle-coupled exports of [WarrenJni] (initLogger, connectTunnel,
 * etc.) intentionally do NOT appear here; they stay app-private.
 */
class WarrenJniBridgeImpl : WarrenJniBridge {
    override fun generateMnemonic(): String = ready { WarrenJni.generateMnemonic() }

    override fun mnemonicPubkeySs58(mnemonic: String): String = ready {
        WarrenJni.mnemonicPubkeySs58(mnemonic)
    }

    override fun fetchVersionInfo(currentVersion: String): WarrenVersionVerdict =
        try {
            val obj =
                Json.parseToJsonElement(ready { WarrenJni.fetchVersionInfo(currentVersion) })
                    .jsonObject
            WarrenVersionVerdict(
                isSupported = obj["supported"]?.jsonPrimitive?.booleanOrNull ?: true,
                latestAvailable = obj["latest"]?.jsonPrimitive?.contentOrNull?.ifEmpty { null },
            )
        } catch (e: IllegalArgumentException) {
            // A malformed envelope reads as "nothing known", the same fail-open
            // and fail-closed pair the native side answers on its own failures.
            WarrenVersionVerdict.UNKNOWN
        }

    override fun fetchNetworkInfo(): String = ready { WarrenJni.fetchNetworkInfo() }

    override fun forumLogin(mnemonic: String, sid: String, host: String): String = ready {
        WarrenJni.forumLogin(mnemonic, sid, host)
    }

    override fun forumLoginCancel(sid: String, host: String) = ready {
        WarrenJni.forumLoginCancel(sid, host)
    }

    override fun forumReport(mnemonic: String, reportJson: String, logGz: ByteArray?): String =
        ready { WarrenJni.forumReport(mnemonic, reportJson, logGz) }

    override fun forumDigestFetch(): String = ready { WarrenJni.forumDigestFetch() }

    override fun noticesFetch(currentVersion: String): String = ready {
        WarrenJni.noticesFetch(currentVersion)
    }

    override fun forumNotifications(mnemonic: String): String = ready {
        WarrenJni.forumNotifications(mnemonic)
    }

    override fun forumNotificationsSeen(mnemonic: String): String = ready {
        WarrenJni.forumNotificationsSeen(mnemonic)
    }

    override fun collectProblemReport(
        metadataJson: String,
        redactJson: String,
        appLogDir: String,
        outputPath: String,
        forSend: Boolean,
    ): String = ready {
        WarrenJni.collectProblemReport(metadataJson, redactJson, appLogDir, outputPath, forSend)
    }

    private inline fun <T> ready(call: () -> T): T {
        WarrenNativeRuntime.awaitReadyBlocking()
        return call()
    }
}
