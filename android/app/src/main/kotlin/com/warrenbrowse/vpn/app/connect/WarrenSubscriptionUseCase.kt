package com.warrenbrowse.vpn.app.connect

import androidx.fragment.app.FragmentActivity
import co.touchlab.kermit.Logger
import com.warrenbrowse.vpn.jni.WarrenJni
import com.warrenbrowse.vpn.jni.WarrenNativeRuntime
import com.warrenbrowse.vpn.lib.model.wallet.WalletState
import com.warrenbrowse.vpn.lib.repository.PurchaseClaim
import com.warrenbrowse.vpn.lib.repository.WalletRepository
import com.warrenbrowse.vpn.lib.repository.WarrenAccountStandingState
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository
import com.warrenbrowse.vpn.lib.repository.WarrenSubscriptionInvoker
import com.warrenbrowse.vpn.lib.repository.WarrenSubscriptionOutcome
import com.warrenbrowse.vpn.lib.repository.WarrenVoucherOutcome
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
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
 * Flow:
 *   1. Require a wallet on device (only Absent blocks).
 *   2. Read the mnemonic silently (routine signing, no prompt).
 *   3. Hand the mnemonic to `WarrenJni.getSubscription`, which signs the
 *      request with the wallet pubkey and GETs `/v1/subscription`.
 *   4. Parse the JSON response into [WarrenSubscriptionOutcome].
 */
class WarrenSubscriptionUseCase(
    private val walletRepository: WalletRepository,
    private val localSettings: WarrenLocalSettingsRepository,
    private val standing: WarrenAccountStandingState,
    private val purchases: PurchaseVoucherKeeper,
) : WarrenSubscriptionInvoker {

    // App-scoped (not tied to any screen) so the purchase poll survives the
    // user navigating away while they pay in the browser, mirroring the desktop
    // poll that lives on the AppRenderer. On the main dispatcher because the
    // unlock step shows a BiometricPrompt.
    private val pollScope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)

    override suspend fun fetch(activity: FragmentActivity): WarrenSubscriptionOutcome {
        // A Locked wallet is fine: unlock() decrypts from disk regardless of
        // state. Only Absent (no wallet) blocks the signed request. (Gating on
        // Ready alone broke fetch/redeem at rest, since the resting state is
        // Locked and only becomes Ready transiently right after create/import.)
        if (walletRepository.state.value is WalletState.Absent) {
            Logger.w("WarrenSubscriptionUseCase: no wallet on device")
            return WarrenSubscriptionOutcome.WalletNotReady
        }

        // Routine signing: read silently, no prompt (see WarrenConnectUseCase).
        val mnemonic = try {
            walletRepository.readMnemonic()
        } catch (e: Exception) {
            Logger.e(throwable = e) { "WarrenSubscriptionUseCase: mnemonic read failed" }
            return WarrenSubscriptionOutcome.Failure(e.message ?: "wallet read failed")
        }

        return withContext(Dispatchers.IO) {
            WarrenNativeRuntime.awaitReadyBlocking()
            mnemonic.use { m ->
                val rawJson = try {
                    WarrenJni.getSubscription(m.phrase)
                } catch (e: Exception) {
                    Logger.e(throwable = e) { "WarrenJni.getSubscription threw" }
                    return@use WarrenSubscriptionOutcome.Failure(
                        e.message ?: "JNI getSubscription threw",
                    )
                }
                parseOutcome(rawJson).also { outcome ->
                    // Cache the freshly fetched expiry into the shared StateFlow so
                    // every observer (account "Paid until", home header "Time
                    // left") refreshes. A 404 resolves to 0 (no subscription),
                    // which correctly clears a stale cached value.
                    if (outcome is WarrenSubscriptionOutcome.Success) {
                        localSettings.setCachedSubscriptionExpiry(outcome.expiresAtUnixSecs)
                    }
                }
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
            walletRepository.readMnemonic()
        } catch (e: Exception) {
            Logger.e(throwable = e) { "WarrenSubscriptionUseCase.redeem: mnemonic read failed" }
            return WarrenVoucherOutcome.Failure(e.message ?: "wallet read failed")
        }

        return withContext(Dispatchers.IO) {
            WarrenNativeRuntime.awaitReadyBlocking()
            mnemonic.use { m ->
                val rawJson = try {
                    WarrenJni.redeemVoucher(m.phrase, voucher)
                } catch (e: Exception) {
                    Logger.e(throwable = e) { "WarrenJni.redeemVoucher threw" }
                    return@use WarrenVoucherOutcome.Failure(e.message ?: "JNI redeemVoucher threw")
                }
                parseVoucherJson(rawJson).also(::showBan)
            }
        }
    }

    /**
     * A redemption the ban refused shows the suspension at once, on the screens
     * that render the standing, without waiting for the next standing poll.
     */
    private fun showBan(outcome: WarrenVoucherOutcome) {
        if (outcome is WarrenVoucherOutcome.Banned) {
            standing.setStanding(withRefusalBan(standing.standing.value, outcome.ban))
        }
    }

    /**
     * App-initiated purchase auto-credit (warren-core doc 35), the Android
     * counterpart of the desktop `buyCredit` poll. The caller opens the
     * checkout bound to the [claim]'s wpid and pull-secret hash; here we read
     * the mnemonic silently (no prompt) and poll the signed
     * `redeemVoucher(claim.code)` every [intervalMs] until the payment webhook
     * has queued the voucher under the wpid (Success) or [deadlineMs] elapses. The signed call is what binds
     * the payment to this wallet; the daemon never learns the pubkey from the
     * checkout URL.
     *
     * The decrypted mnemonic is held for the poll window, then zeroed when the
     * poll ends.
     */
    override fun startPurchasePoll(
        activity: FragmentActivity,
        claim: PurchaseClaim,
        intervalMs: Long,
        deadlineMs: Long,
    ) {
        pollScope.launch {
            if (walletRepository.state.value is WalletState.Absent) return@launch
            // Routine signing: read silently, no prompt (see WarrenConnectUseCase).
            val mnemonic = try {
                walletRepository.readMnemonic()
            } catch (e: Exception) {
                Logger.e(throwable = e) { "WarrenSubscriptionUseCase.poll: mnemonic read failed" }
                return@launch
            }

            // try/finally (not use {}) so delay() can suspend between attempts
            // while still zeroing the mnemonic when the poll ends or is cancelled.
            try {
                val deadline = System.currentTimeMillis() + deadlineMs
                while (System.currentTimeMillis() < deadline) {
                    val outcome = withContext(Dispatchers.IO) {
                        WarrenNativeRuntime.awaitReadyBlocking()
                        try {
                            purchases.collect(claim.code, mnemonic.phrase)
                        } catch (e: Exception) {
                            WarrenVoucherOutcome.Failure(e.message ?: "JNI purchase collect threw")
                        }
                    }
                    when (outcome) {
                        // The answer will not change within this poll. The
                        // pulled voucher waits, sealed, for the ban to end.
                        is WarrenVoucherOutcome.Banned -> {
                            showBan(outcome)
                            return@launch
                        }
                        is WarrenVoucherOutcome.Success -> {
                            // Write the credited expiry to the shared StateFlow so
                            // every screen observing it (account "Paid until", the
                            // home header "Time left") refreshes automatically.
                            localSettings.setCachedSubscriptionExpiry(outcome.expiresAtUnixSecs)
                            return@launch
                        }
                        WarrenVoucherOutcome.Rejected -> return@launch
                        else -> Unit
                    }
                    delay(intervalMs)
                }
            } finally {
                mnemonic.close()
            }
        }
    }

    /**
     * Redeems the vouchers pulled for this wallet that a ban, or a restart, left waiting in the
     * sealed pending voucher store. Silent: no prompt, and nothing to do without a held voucher.
     */
    // The Keystore read and the JNI call are system boundaries: whatever they throw is one round
    // skipped, retried at the next start or the next end of a ban, never a crash.
    @Suppress("TooGenericExceptionCaught")
    suspend fun redeemHeldVouchers() {
        val wallet = walletAddress()
        if (wallet == null || !withContext(Dispatchers.IO) { purchases.holds(wallet) }) return
        val mnemonic = try {
            walletRepository.readMnemonic()
        } catch (e: Exception) {
            Logger.w { "redeemHeldVouchers: wallet unreadable (${e::class.simpleName})" }
            null
        } ?: return
        val outcomes = withContext(Dispatchers.IO) {
            WarrenNativeRuntime.awaitReadyBlocking()
            mnemonic.use { m ->
                try {
                    purchases.redeemHeld(m.phrase)
                } catch (e: Exception) {
                    Logger.w { "redeemHeldVouchers failed (${e::class.simpleName})" }
                    emptyList()
                }
            }
        }
        outcomes.forEach { outcome ->
            when (outcome) {
                is WarrenVoucherOutcome.Success ->
                    localSettings.setCachedSubscriptionExpiry(outcome.expiresAtUnixSecs)
                is WarrenVoucherOutcome.Banned -> showBan(outcome)
                else -> Unit
            }
        }
    }

    private fun walletAddress(): String? =
        when (val state = walletRepository.state.value) {
            is WalletState.Locked -> state.pubkey.value
            is WalletState.Ready -> state.pubkey.value
            else -> null
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
