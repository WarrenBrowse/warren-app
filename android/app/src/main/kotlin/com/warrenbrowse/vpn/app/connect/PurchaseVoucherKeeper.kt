package com.warrenbrowse.vpn.app.connect

import co.touchlab.kermit.Logger
import com.warrenbrowse.vpn.jni.WarrenJni
import com.warrenbrowse.vpn.lib.repository.PendingVoucher
import com.warrenbrowse.vpn.lib.repository.PendingVoucherStore
import com.warrenbrowse.vpn.lib.repository.WarrenVoucherOutcome
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.contentOrNull

/** The two `WarrenJni` calls a purchase is collected with. Blocking: call them off the main thread. */
interface PurchaseVoucherBridge {
    /** `WarrenJni.pullPurchaseVoucher`: the voucher a purchase paid for, unredeemed. */
    fun pullPurchaseVoucher(claimCode: String): String

    /** `WarrenJni.redeemVoucher`. */
    fun redeemVoucher(mnemonic: String, voucher: String): String
}

object WarrenJniPurchaseVoucherBridge : PurchaseVoucherBridge {
    override fun pullPurchaseVoucher(claimCode: String): String =
        WarrenJni.pullPurchaseVoucher(claimCode)

    override fun redeemVoucher(mnemonic: String, voucher: String): String =
        WarrenJni.redeemVoucher(mnemonic, voucher)
}

/** What `WarrenJni.pullPurchaseVoucher` answered. */
sealed interface PurchasePull {
    data class Pulled(val voucher: String) : PurchasePull {
        // The voucher is a bearer secret: never through interpolation into a log line.
        override fun toString(): String = "Pulled([redacted])"
    }

    /** The payment has queued no voucher yet: the steady state of a purchase poll. */
    data object NotReady : PurchasePull

    data object Failed : PurchasePull
}

/**
 * Reads `{"ok":true,"voucher":".."}`, `{"ok":false,"error":"purchase pending"}` or any other
 * refusal. The decoder's message is never logged: the input holds the voucher.
 */
internal fun parsePullJson(rawJson: String): PurchasePull =
    try {
        val root = Json.parseToJsonElement(rawJson) as? JsonObject
        val ok = (root?.get("ok") as? JsonPrimitive)?.booleanOrNull == true
        val voucher = (root?.get("voucher") as? JsonPrimitive)?.contentOrNull
        val error = (root?.get("error") as? JsonPrimitive)?.contentOrNull
        when {
            ok && !voucher.isNullOrEmpty() -> PurchasePull.Pulled(voucher)
            error == "purchase pending" -> PurchasePull.NotReady
            else -> PurchasePull.Failed
        }
    } catch (e: IllegalArgumentException) {
        Logger.w { "pullPurchaseVoucher answered a malformed envelope (${e::class.simpleName})" }
        PurchasePull.Failed
    }

/**
 * Collects the vouchers app-initiated purchases paid for (warren-core doc 35) so that none is
 * ever lost: the server hands a voucher out once, and a ban refuses its redemption for up to a
 * year while leaving it unspent (doc 105 section 5.3). Each pulled voucher is sealed in [store]
 * before its first redemption, and leaves it only once redeemed or refused for good.
 */
class PurchaseVoucherKeeper(
    private val bridge: PurchaseVoucherBridge,
    private val store: PendingVoucherStore,
) {
    /**
     * One step of a purchase poll: pulls the voucher [claimCode] paid for, holds it for
     * [wallet], then redeems it. `null` while the payment has queued nothing, or the pull
     * failed: the poll asks again.
     */
    fun collect(claimCode: String, wallet: String, phrase: String): WarrenVoucherOutcome? {
        val pulled = parsePullJson(bridge.pullPurchaseVoucher(claimCode))
        if (pulled !is PurchasePull.Pulled) return null
        val pending = PendingVoucher(wallet, pulled.voucher)
        store.hold(pending)
        return redeem(pending, phrase)
    }

    /**
     * Redeems every voucher held for [wallet], and stops at the first ban: the others would be
     * refused the same way. Vouchers held for another wallet wait for it.
     */
    fun redeemHeld(wallet: String, phrase: String): List<WarrenVoucherOutcome> {
        val outcomes = mutableListOf<WarrenVoucherOutcome>()
        for (pending in store.all().filter { it.wallet == wallet }) {
            val outcome = redeem(pending, phrase)
            outcomes += outcome
            if (outcome is WarrenVoucherOutcome.Banned) break
        }
        return outcomes
    }

    /** Whether a voucher waits for [wallet]. */
    fun holds(wallet: String): Boolean = store.all().any { it.wallet == wallet }

    private fun redeem(pending: PendingVoucher, phrase: String): WarrenVoucherOutcome {
        val outcome = parseVoucherJson(bridge.redeemVoucher(phrase, pending.voucher))
        if (outcome is WarrenVoucherOutcome.Success || outcome is WarrenVoucherOutcome.Rejected) {
            store.forget(pending.voucher)
        }
        return outcome
    }
}
