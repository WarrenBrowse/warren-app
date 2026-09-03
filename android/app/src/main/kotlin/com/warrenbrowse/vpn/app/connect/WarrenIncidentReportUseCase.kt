package com.warrenbrowse.vpn.app.connect

import co.touchlab.kermit.Logger
import com.warrenbrowse.vpn.lib.model.wallet.Mnemonic
import com.warrenbrowse.vpn.lib.model.wallet.WalletState
import com.warrenbrowse.vpn.lib.repository.WalletRepository
import com.warrenbrowse.vpn.lib.repository.WarrenIncidentReporter
import com.warrenbrowse.vpn.lib.repository.WarrenJniBridge
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive

/**
 * The key-mismatch report the user asks for, mirroring the desktop's "Report to Warren" button on
 * the same dialog. The wallet signs it and the POST leaves inside Rust; this side resolves the
 * mnemonic silently (a routine signing op, no biometric prompt), hands over the facts and maps the
 * envelope to a yes or no.
 *
 * Nothing here can fail loudly: the user has already been told the connect was refused, and a lost
 * forensic point must not turn into an error on top of a security warning.
 */
class WarrenIncidentReportUseCase(
    private val walletRepository: WalletRepository,
    private val jni: WarrenJniBridge,
) : WarrenIncidentReporter {

    @Suppress("TooGenericExceptionCaught")
    override suspend fun reportPubkeyMismatch(
        exitIdHex: String,
        oldPubkeyHex: String,
        newPubkeyHex: String,
        countryCode: String,
        city: String,
    ): Boolean {
        val mnemonic = mnemonicOrNull() ?: return false
        val envelope =
            withContext(Dispatchers.IO) {
                mnemonic.use { m ->
                    try {
                        jni.reportPubkeyMismatch(
                            m.phrase,
                            exitIdHex,
                            oldPubkeyHex,
                            newPubkeyHex,
                            countryCode,
                            city,
                        )
                    } catch (e: Exception) {
                        Logger.e(throwable = e) { "WarrenJniBridge.reportPubkeyMismatch threw" }
                        null
                    }
                }
            }
        return envelope?.let(::parseIncidentEnvelope) == true
    }

    /** The phrase to sign this one report with, or null when there is none to read. */
    @Suppress("TooGenericExceptionCaught")
    private suspend fun mnemonicOrNull(): Mnemonic? =
        if (walletRepository.state.value is WalletState.Absent) {
            Logger.w("WarrenIncidentReportUseCase: no wallet, nothing to sign the report with")
            null
        } else {
            try {
                walletRepository.readMnemonic()
            } catch (e: Exception) {
                Logger.e(throwable = e) { "reportPubkeyMismatch: mnemonic read failed" }
                null
            }
        }
}

/**
 * Read the `{"ok":..}` incident envelope. A malformed one reads as "did not leave", the same
 * direction every failure class takes. Pure, so it is unit-testable off-device.
 */
internal fun parseIncidentEnvelope(rawJson: String): Boolean =
    try {
        Json.parseToJsonElement(rawJson).jsonObject["ok"]?.jsonPrimitive?.booleanOrNull == true
    } catch (e: IllegalArgumentException) {
        Logger.w(throwable = e) { "reportPubkeyMismatch: unreadable envelope" }
        false
    }
