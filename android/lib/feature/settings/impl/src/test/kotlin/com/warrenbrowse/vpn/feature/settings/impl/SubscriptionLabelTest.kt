package com.warrenbrowse.vpn.feature.settings.impl

import android.content.Context
import com.warrenbrowse.vpn.lib.repository.WarrenSubscriptionOutcome
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
        every { getString(R.string.subscription_none_active) } returns "No active subscription."
        every { getString(R.string.subscription_authorization_cancelled) } returns
            "Authorization cancelled."
        every { getString(R.string.subscription_wallet_not_ready) } returns
            "Set up your wallet first."
        every { getString(R.string.subscription_fetch_failed) } returns
            "Couldn't fetch subscription status."
        every { getString(R.string.subscription_voucher_redeem_failed) } returns
            "Couldn't redeem voucher. Check the code and try again."
        every { getString(eq(R.string.subscription_active_expires), any()) } answers
            { "Subscription active - expires ${fmtArgs()[0]}" }
        every { getString(eq(R.string.subscription_expired), any()) } answers
            { "Subscription expired (${fmtArgs()[0]})" }
        every { getString(eq(R.string.subscription_voucher_redeemed), any()) } answers
            { "Voucher redeemed - subscription expires ${fmtArgs()[0]}" }
        every { getString(eq(R.string.subscription_expired_on), any()) } answers
            { "Subscription expired on ${fmtArgs()[0]}" }
        every { getString(eq(R.string.subscription_expires_in_day), any(), any()) } answers
            { "Subscription expires in ${fmtArgs()[0]} day (${fmtArgs()[1]})" }
        every { getString(eq(R.string.subscription_expires_in_days), any(), any()) } answers
            { "Subscription expires in ${fmtArgs()[0]} days (${fmtArgs()[1]})" }
    }

    @Test
    fun `active subscription when expiry is in the future`() {
        val label = subscriptionLabel(
            context,
            WarrenSubscriptionOutcome.Success(expiresAtUnixSecs = 2_000),
            nowSecs = 1_000,
        )
        assertTrue(label.startsWith("Subscription active"), label)
    }

    @Test
    fun `expired subscription when expiry is in the past`() {
        val label = subscriptionLabel(
            context,
            WarrenSubscriptionOutcome.Success(expiresAtUnixSecs = 1_000),
            nowSecs = 2_000,
        )
        assertTrue(label.startsWith("Subscription expired"), label)
    }

    @Test
    fun `no active subscription when expiry is the epoch sentinel (404)`() {
        // A 404 from the subscription endpoint resolves to epoch (0), which
        // must read as "no active subscription", not "expired (1970)".
        val label = subscriptionLabel(
            context,
            WarrenSubscriptionOutcome.Success(expiresAtUnixSecs = 0),
            nowSecs = 2_000,
        )
        assertEquals("No active subscription.", label)
    }

    @Test
    fun `voucher success renders the new expiry, failures are fixed`() {
        assertTrue(
            voucherLabel(context, WarrenVoucherOutcome.Success(2_000)).startsWith("Voucher redeemed"),
        )
        assertEquals(
            "Authorization cancelled.",
            voucherLabel(context, WarrenVoucherOutcome.AuthorizationDenied),
        )
        assertEquals(
            "Couldn't redeem voucher. Check the code and try again.",
            voucherLabel(context, WarrenVoucherOutcome.Failure("invalid")),
        )
    }

    @Test
    fun `cached subscription label is null when expiry unknown`() {
        assertEquals(null, cachedSubscriptionLabel(context, 0L, nowSecs = 1_000))
    }

    @Test
    fun `cached subscription label warns within the expiry window and reports days`() {
        val now = 1_000_000L
        val label = cachedSubscriptionLabel(context, now + 3 * 86_400, nowSecs = now)
        assertTrue(label != null && label.startsWith("Subscription expires in 3 days"), "$label")
    }

    @Test
    fun `cached subscription label is active when far from expiry, expired when past`() {
        val now = 1_000_000L
        assertTrue(
            cachedSubscriptionLabel(context, now + 60 * 86_400, nowSecs = now)!!
                .startsWith("Subscription active"),
        )
        assertTrue(
            cachedSubscriptionLabel(context, now - 86_400, nowSecs = now)!!
                .startsWith("Subscription expired on"),
        )
    }

    @Test
    fun `non-success outcomes map to fixed messages`() {
        assertEquals(
            "Authorization cancelled.",
            subscriptionLabel(context, WarrenSubscriptionOutcome.AuthorizationDenied),
        )
        assertEquals(
            "Set up your wallet first.",
            subscriptionLabel(context, WarrenSubscriptionOutcome.WalletNotReady),
        )
        assertEquals(
            "Couldn't fetch subscription status.",
            subscriptionLabel(context, WarrenSubscriptionOutcome.Failure("boom 500")),
        )
    }
}
