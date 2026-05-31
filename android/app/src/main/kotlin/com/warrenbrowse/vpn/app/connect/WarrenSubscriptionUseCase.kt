package com.warrenbrowse.vpn.app.connect

import androidx.fragment.app.FragmentActivity
import co.touchlab.kermit.Logger
import com.warrenbrowse.vpn.jni.WarrenJni
import com.warrenbrowse.vpn.lib.model.wallet.WalletState
import com.warrenbrowse.vpn.lib.repository.WalletAuthorizationDeniedException
import com.warrenbrowse.vpn.lib.repository.WalletRepository
import com.warrenbrowse.vpn.lib.repository.WarrenSubscriptionInvoker
import com.warrenbrowse.vpn.lib.repository.WarrenSubscriptionOutcome
import com.warrenbrowse.vpn.lib.repository.WarrenVoucherOutcome
import com.warrenbrowse.vpn.lib.ui.component.wallet.BiometricPromptAuthorizer
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.boolean
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.long

/**
 * Fetches the wallet's subscription status (signed `GET /v1/subscription`).
 *
 * Mirrors [WarrenSendProblemReportUseCase]:
 *   1. Require wallet [WalletState.Ready].
 *   2. Unlock via biometric prompt to obtain the mnemonic.
 *   3. Hand the mnemonic to `WarrenJni.getSubscription`, which signs the
 *      request with the wallet pubkey and GETs `/v1/subscription`.
 *   4. Parse the JSON response into [WarrenSubscriptionOutcome].
 */
class WarrenSubscriptionUseCase(
    private val walletRepository: WalletRepository,
) : WarrenSubscriptionInvoker {

    override suspend fun fetch(activity: FragmentActivity): WarrenSubscriptionOutcome {
        val state = walletRepository.state.value
        if (state !is WalletState.Ready) {
            Logger.w("WarrenSubscriptionUseCase: wallet not Ready (state=$state)")
            return WarrenSubscriptionOutcome.WalletNotReady
        }

        val authorizer = BiometricPromptAuthorizer(activity)
        val mnemonic = try {
            walletRepository.unlock(
                authorizer = authorizer,
                reason = "Check your subscription",
            )
        } catch (e: WalletAuthorizationDeniedException) {
            return WarrenSubscriptionOutcome.AuthorizationDenied
        } catch (e: Exception) {
            Logger.e(throwable = e) { "WarrenSubscriptionUseCase: unlock failed" }
            return WarrenSubscriptionOutcome.Failure(e.message ?: "wallet unlock failed")
        }

        return withContext(Dispatchers.IO) {
            mnemonic.use { m ->
                val rawJson = try {
                    WarrenJni.getSubscription(m.phrase)
                } catch (e: Exception) {
                    Logger.e(throwable = e) { "WarrenJni.getSubscription threw" }
                    return@use WarrenSubscriptionOutcome.Failure(
                        e.message ?: "JNI getSubscription threw",
                    )
                }
                parseOutcome(rawJson)
            }
        }
    }

    override suspend fun redeemVoucher(
        activity: FragmentActivity,
        voucher: String,
    ): WarrenVoucherOutcome {
        val state = walletRepository.state.value
        if (state !is WalletState.Ready) {
            Logger.w("WarrenSubscriptionUseCase.redeem: wallet not Ready (state=$state)")
            return WarrenVoucherOutcome.WalletNotReady
        }
        val mnemonic = try {
            walletRepository.unlock(
                authorizer = BiometricPromptAuthorizer(activity),
                reason = "Redeem your voucher",
            )
        } catch (e: WalletAuthorizationDeniedException) {
            return WarrenVoucherOutcome.AuthorizationDenied
        } catch (e: Exception) {
            Logger.e(throwable = e) { "WarrenSubscriptionUseCase.redeem: unlock failed" }
            return WarrenVoucherOutcome.Failure(e.message ?: "wallet unlock failed")
        }

        return withContext(Dispatchers.IO) {
            mnemonic.use { m ->
                val rawJson = try {
                    WarrenJni.redeemVoucher(m.phrase, voucher)
                } catch (e: Exception) {
                    Logger.e(throwable = e) { "WarrenJni.redeemVoucher threw" }
                    return@use WarrenVoucherOutcome.Failure(e.message ?: "JNI redeemVoucher threw")
                }
                try {
                    val root = Json.parseToJsonElement(rawJson).jsonObject
                    if (root["ok"]?.jsonPrimitive?.boolean == true) {
                        val expiresAt = root["expires_at"]?.jsonPrimitive?.long
                            ?: return@use WarrenVoucherOutcome.Failure("missing expires_at")
                        WarrenVoucherOutcome.Success(expiresAt)
                    } else {
                        WarrenVoucherOutcome.Failure(
                            root["error"]?.jsonPrimitive?.content ?: "redeem failed",
                        )
                    }
                } catch (e: Exception) {
                    WarrenVoucherOutcome.Failure("invalid JNI response: ${e.message}")
                }
            }
        }
    }

    private fun parseOutcome(rawJson: String): WarrenSubscriptionOutcome = try {
        val root = Json.parseToJsonElement(rawJson).jsonObject
        val ok = root["ok"]?.jsonPrimitive?.boolean == true
        if (ok) {
            val expiresAt = root["expires_at"]?.jsonPrimitive?.long
                ?: return WarrenSubscriptionOutcome.Failure("missing expires_at")
            WarrenSubscriptionOutcome.Success(expiresAt)
        } else {
            val msg = root["error"]?.jsonPrimitive?.content ?: "fetch failed"
            WarrenSubscriptionOutcome.Failure(msg)
        }
    } catch (e: Exception) {
        WarrenSubscriptionOutcome.Failure("invalid JNI response: ${e.message}")
    }
}
