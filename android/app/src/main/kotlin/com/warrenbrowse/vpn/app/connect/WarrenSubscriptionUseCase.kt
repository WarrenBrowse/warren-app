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
import kotlinx.coroutines.delay
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.boolean
import kotlinx.serialization.json.intOrNull
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
        // A Locked wallet is fine: unlock() decrypts from disk regardless of
        // state. Only Absent (no wallet) blocks the signed request. (Gating on
        // Ready alone broke fetch/redeem at rest, since the resting state is
        // Locked and only becomes Ready transiently right after create/import.)
        if (walletRepository.state.value is WalletState.Absent) {
            Logger.w("WarrenSubscriptionUseCase: no wallet on device")
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
        if (walletRepository.state.value is WalletState.Absent) {
            Logger.w("WarrenSubscriptionUseCase.redeem: no wallet on device")
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
                parseVoucherJson(rawJson)
            }
        }
    }

    /**
     * App-initiated purchase auto-credit (warren-core doc 35), the Android
     * counterpart of the desktop `buyCredit` poll. The caller opens the
     * checkout bound to a random 32-hex purchase id (wpid); here we unlock the
     * wallet ONCE (single biometric/credential prompt) and then poll the signed
     * `redeemVoucher(wpid)` every [intervalMs] until the payment webhook has
     * queued the voucher under that id (Success) or [deadlineMs] elapses. The
     * signed call is what binds the payment to this wallet; the daemon never
     * learns the pubkey from the checkout URL.
     *
     * The decrypted mnemonic is held for the poll window so we don't re-prompt
     * on every attempt, then zeroed when the poll ends.
     */
    override suspend fun pollPurchase(
        activity: FragmentActivity,
        wpid: String,
        intervalMs: Long,
        deadlineMs: Long,
    ): WarrenVoucherOutcome {
        if (walletRepository.state.value is WalletState.Absent) {
            return WarrenVoucherOutcome.WalletNotReady
        }
        val mnemonic = try {
            walletRepository.unlock(
                authorizer = BiometricPromptAuthorizer(activity),
                reason = "Confirm to credit your purchase",
            )
        } catch (e: WalletAuthorizationDeniedException) {
            return WarrenVoucherOutcome.AuthorizationDenied
        } catch (e: Exception) {
            Logger.e(throwable = e) { "WarrenSubscriptionUseCase.poll: unlock failed" }
            return WarrenVoucherOutcome.Failure(e.message ?: "wallet unlock failed")
        }

        // try/finally (not use {}) so we can suspend on delay() between attempts
        // while still zeroing the mnemonic when the poll completes or is cancelled.
        return try {
            val deadline = System.currentTimeMillis() + deadlineMs
            var last: WarrenVoucherOutcome = WarrenVoucherOutcome.Failure("not credited yet")
            while (System.currentTimeMillis() < deadline) {
                val outcome = withContext(Dispatchers.IO) {
                    try {
                        parseVoucherJson(WarrenJni.redeemVoucher(mnemonic.phrase, wpid))
                    } catch (e: Exception) {
                        WarrenVoucherOutcome.Failure(e.message ?: "JNI redeemVoucher threw")
                    }
                }
                if (outcome is WarrenVoucherOutcome.Success) return outcome
                last = outcome
                delay(intervalMs)
            }
            last
        } finally {
            mnemonic.close()
        }
    }

    private fun parseVoucherJson(rawJson: String): WarrenVoucherOutcome = try {
        val root = Json.parseToJsonElement(rawJson).jsonObject
        if (root["ok"]?.jsonPrimitive?.boolean == true) {
            val expiresAt = root["expires_at"]?.jsonPrimitive?.long
                ?: return WarrenVoucherOutcome.Failure("missing expires_at")
            WarrenVoucherOutcome.Success(expiresAt)
        } else {
            WarrenVoucherOutcome.Failure(root["error"]?.jsonPrimitive?.content ?: "redeem failed")
        }
    } catch (e: Exception) {
        WarrenVoucherOutcome.Failure("invalid JNI response: ${e.message}")
    }

    private fun parseOutcome(rawJson: String): WarrenSubscriptionOutcome = try {
        val root = Json.parseToJsonElement(rawJson).jsonObject
        val ok = root["ok"]?.jsonPrimitive?.boolean == true
        if (ok) {
            val expiresAt = root["expires_at"]?.jsonPrimitive?.long
                ?: return WarrenSubscriptionOutcome.Failure("missing expires_at")
            WarrenSubscriptionOutcome.Success(expiresAt)
        } else if (root["status"]?.jsonPrimitive?.intOrNull == 404) {
            // A 404 means "no subscription bound yet" (a fresh wallet), not a
            // failure. Mirror iOS / desktop by resolving it to the Unix epoch
            // so the UI shows "no active subscription" rather than a fetch
            // error (the JNI surfaces the HTTP status for exactly this case).
            WarrenSubscriptionOutcome.Success(0L)
        } else {
            val msg = root["error"]?.jsonPrimitive?.content ?: "fetch failed"
            WarrenSubscriptionOutcome.Failure(msg)
        }
    } catch (e: Exception) {
        WarrenSubscriptionOutcome.Failure("invalid JNI response: ${e.message}")
    }
}
