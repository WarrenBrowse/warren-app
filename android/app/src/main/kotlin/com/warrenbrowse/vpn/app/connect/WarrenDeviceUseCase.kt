package com.warrenbrowse.vpn.app.connect

import androidx.fragment.app.FragmentActivity
import co.touchlab.kermit.Logger
import com.warrenbrowse.vpn.jni.WarrenJni
import com.warrenbrowse.vpn.lib.model.wallet.WalletState
import com.warrenbrowse.vpn.lib.repository.WalletAuthorizationDeniedException
import com.warrenbrowse.vpn.lib.repository.WalletRepository
import com.warrenbrowse.vpn.lib.repository.WarrenDeviceInvoker
import com.warrenbrowse.vpn.lib.repository.WarrenDeviceListOutcome
import com.warrenbrowse.vpn.lib.repository.WarrenDeviceRemoveOutcome
import com.warrenbrowse.vpn.lib.repository.WarrenDeviceSummary
import com.warrenbrowse.vpn.lib.ui.component.wallet.BiometricPromptAuthorizer
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.boolean
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.long

/**
 * Lists and removes the wallet's registered devices. Each call unlocks
 * the mnemonic via biometric prompt and hands it to the device JNI
 * exports, which sign the request with the wallet pubkey. Mirrors
 * [WarrenSubscriptionUseCase].
 */
class WarrenDeviceUseCase(
    private val walletRepository: WalletRepository,
) : WarrenDeviceInvoker {

    override suspend fun list(activity: FragmentActivity): WarrenDeviceListOutcome {
        val state = walletRepository.state.value
        if (state !is WalletState.Ready) {
            Logger.w("WarrenDeviceUseCase.list: wallet not Ready (state=$state)")
            return WarrenDeviceListOutcome.WalletNotReady
        }
        val mnemonic = try {
            walletRepository.unlock(
                authorizer = BiometricPromptAuthorizer(activity),
                reason = "List your devices",
            )
        } catch (e: WalletAuthorizationDeniedException) {
            return WarrenDeviceListOutcome.AuthorizationDenied
        } catch (e: Exception) {
            Logger.e(throwable = e) { "WarrenDeviceUseCase.list: unlock failed" }
            return WarrenDeviceListOutcome.Failure(e.message ?: "wallet unlock failed")
        }

        return withContext(Dispatchers.IO) {
            mnemonic.use { m ->
                val rawJson = try {
                    WarrenJni.listDevices(m.phrase)
                } catch (e: Exception) {
                    Logger.e(throwable = e) { "WarrenJni.listDevices threw" }
                    return@use WarrenDeviceListOutcome.Failure(e.message ?: "JNI listDevices threw")
                }
                parseListOutcome(rawJson)
            }
        }
    }

    override suspend fun remove(
        activity: FragmentActivity,
        deviceId: String,
    ): WarrenDeviceRemoveOutcome {
        val state = walletRepository.state.value
        if (state !is WalletState.Ready) {
            Logger.w("WarrenDeviceUseCase.remove: wallet not Ready (state=$state)")
            return WarrenDeviceRemoveOutcome.WalletNotReady
        }
        val mnemonic = try {
            walletRepository.unlock(
                authorizer = BiometricPromptAuthorizer(activity),
                reason = "Remove this device",
            )
        } catch (e: WalletAuthorizationDeniedException) {
            return WarrenDeviceRemoveOutcome.AuthorizationDenied
        } catch (e: Exception) {
            Logger.e(throwable = e) { "WarrenDeviceUseCase.remove: unlock failed" }
            return WarrenDeviceRemoveOutcome.Failure(e.message ?: "wallet unlock failed")
        }

        return withContext(Dispatchers.IO) {
            mnemonic.use { m ->
                val rawJson = try {
                    WarrenJni.removeDevice(m.phrase, deviceId)
                } catch (e: Exception) {
                    Logger.e(throwable = e) { "WarrenJni.removeDevice threw" }
                    return@use WarrenDeviceRemoveOutcome.Failure(e.message ?: "JNI removeDevice threw")
                }
                val ok = try {
                    Json.parseToJsonElement(rawJson).jsonObject["ok"]?.jsonPrimitive?.boolean == true
                } catch (e: Exception) {
                    false
                }
                if (ok) WarrenDeviceRemoveOutcome.Success
                else WarrenDeviceRemoveOutcome.Failure("remove failed")
            }
        }
    }

    private fun parseListOutcome(rawJson: String): WarrenDeviceListOutcome = try {
        val root = Json.parseToJsonElement(rawJson).jsonObject
        val ok = root["ok"]?.jsonPrimitive?.boolean == true
        if (!ok) {
            WarrenDeviceListOutcome.Failure(
                root["error"]?.jsonPrimitive?.content ?: "list failed",
            )
        } else {
            val devices = root["devices"]?.jsonArray.orEmpty().map { element ->
                val obj = element.jsonObject
                WarrenDeviceSummary(
                    id = obj["id"]?.jsonPrimitive?.content.orEmpty(),
                    name = obj["name"]?.jsonPrimitive?.content.orEmpty(),
                    createdAtUnixSecs = obj["created_at"]?.jsonPrimitive?.long ?: 0L,
                )
            }
            WarrenDeviceListOutcome.Success(devices)
        }
    } catch (e: Exception) {
        WarrenDeviceListOutcome.Failure("invalid JNI response: ${e.message}")
    }
}
