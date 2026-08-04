package com.warrenbrowse.vpn.feature.settings.impl

import android.content.Context
import com.warrenbrowse.vpn.lib.repository.WarrenVoucherOutcome
import com.warrenbrowse.vpn.lib.ui.resource.R
import io.mockk.MockKAnswerScope
import io.mockk.every
import io.mockk.mockk
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import org.junit.jupiter.api.Test

// getString(int, vararg Any) delivers its format args as a single Array in
// invocation.args[1] (mockk does not flatten varargs); fall back to flattened
// args defensively.
private fun MockKAnswerScope<String, *>.fmtArgs(): List<Any?> =
    (invocation.args.getOrNull(1) as? Array<*>)?.toList() ?: invocation.args.drop(1)

class SubscriptionLabelTest {

    // A Context whose getString resolves to the English templates, so these
    // pure-logic tests stay JVM-only (no Android runtime) while still
    // verifying which string resource each branch selects and how it formats.
    private val context: Context = mockk {
        every { getString(R.string.subscription_authorization_cancelled) } returns
            "Authorization cancelled."
        every { getString(R.string.subscription_wallet_not_ready) } returns
            "Set up your wallet first."
        every { getString(R.string.subscription_voucher_redeem_failed) } returns
            "Couldn't redeem voucher. Check the code and try again."
        every { getString(eq(R.string.subscription_voucher_redeemed), any()) } answers
            { "Voucher redeemed - subscription expires ${fmtArgs()[0]}" }
    }

    @Test
    fun `voucher success renders the new expiry`() {
        assertTrue(
            voucherLabel(context, WarrenVoucherOutcome.Success(2_000)).startsWith("Voucher redeemed"),
        )
    }

    @Test
    fun `voucher failures map to fixed messages`() {
        assertEquals(
            "Authorization cancelled.",
            voucherLabel(context, WarrenVoucherOutcome.AuthorizationDenied),
        )
        assertEquals(
            "Set up your wallet first.",
            voucherLabel(context, WarrenVoucherOutcome.WalletNotReady),
        )
        // The JNI voucher result carries no typed invalid/used/expired verdict,
        // so every failure collapses to the single generic message on purpose.
        assertEquals(
            "Couldn't redeem voucher. Check the code and try again.",
            voucherLabel(context, WarrenVoucherOutcome.Failure("invalid")),
        )
    }
}
