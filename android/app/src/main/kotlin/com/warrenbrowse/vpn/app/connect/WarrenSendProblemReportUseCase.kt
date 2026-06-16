package com.warrenbrowse.vpn.app.connect

import android.os.Build
import androidx.fragment.app.FragmentActivity
import co.touchlab.kermit.Logger
import com.warrenbrowse.vpn.BuildConfig
import com.warrenbrowse.vpn.jni.WarrenJni
import com.warrenbrowse.vpn.lib.model.wallet.WalletState
import com.warrenbrowse.vpn.lib.repository.WalletRepository
import com.warrenbrowse.vpn.lib.repository.WarrenSupportReportInvoker
import com.warrenbrowse.vpn.lib.repository.WarrenSupportReportOutcome
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.boolean
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive

/**
 * End-to-end Warren support-report orchestrator (D.6).
 *
 * Mirrors [WarrenConnectUseCase]:
 *   1. Require a wallet on device (only Absent blocks).
 *   2. Read the mnemonic silently (routine signing, no prompt).
 *   3. Hand the mnemonic + payload to `WarrenJni.sendProblemReport`,
 *      which signs the canonical request with the wallet pubkey and
 *      POSTs `/v1/support`.
 *   4. Parse the JSON response into [WarrenSupportReportOutcome].
 */
class WarrenSendProblemReportUseCase(
    private val walletRepository: WalletRepository,
) : WarrenSupportReportInvoker {

    override suspend fun submit(
        activity: FragmentActivity,
        userMessage: String,
        redactedLogs: String,
    ): WarrenSupportReportOutcome {
        // Locked is fine: unlock() decrypts from disk. Only Absent blocks.
        if (walletRepository.state.value is WalletState.Absent) {
            Logger.w("WarrenSendProblemReportUseCase: no wallet on device")
            return WarrenSupportReportOutcome.WalletNotReady
        }

        // Routine signing: read silently, no prompt (see WarrenConnectUseCase).
        val mnemonic = try {
            walletRepository.readMnemonic()
        } catch (e: Exception) {
            Logger.e(throwable = e) { "WarrenSendProblemReportUseCase: mnemonic read failed" }
            return WarrenSupportReportOutcome.Failure(e.message ?: "wallet read failed")
        }

        val appVersion = BuildConfig.VERSION_NAME
        val platform = "android-${Build.SUPPORTED_ABIS.firstOrNull() ?: "unknown"}"

        return withContext(Dispatchers.IO) {
            // `use { }` zeros the underlying CharArray when the lambda
            // exits (whether `sendProblemReport` succeeded or threw),
            // bounding the mnemonic's heap residency to the JNI call.
            mnemonic.use { m ->
                val rawJson = try {
                    WarrenJni.sendProblemReport(
                        mnemonic = m.phrase,
                        userMessage = userMessage,
                        redactedLogs = redactedLogs,
                        appVersion = appVersion,
                        platform = platform,
                    )
                } catch (e: Exception) {
                    Logger.e(throwable = e) { "WarrenJni.sendProblemReport threw" }
                    return@use WarrenSupportReportOutcome.Failure(
                        e.message ?: "JNI sendProblemReport threw",
                    )
                }
                parseOutcome(rawJson)
            }
        }
    }

    private fun parseOutcome(rawJson: String): WarrenSupportReportOutcome = try {
        val root = Json.parseToJsonElement(rawJson).jsonObject
        val ok = root["ok"]?.jsonPrimitive?.boolean == true
        if (ok) {
            val ref = root["reference_id"]?.jsonPrimitive?.content
                ?: return WarrenSupportReportOutcome.Failure("missing reference_id")
            WarrenSupportReportOutcome.Success(ref)
        } else {
            val msg = root["error"]?.jsonPrimitive?.content ?: "send failed"
            WarrenSupportReportOutcome.Failure(msg)
        }
    } catch (e: Exception) {
        WarrenSupportReportOutcome.Failure("invalid JNI response: ${e.message}")
    }
}
