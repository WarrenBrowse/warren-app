package com.warrenbrowse.vpn.feature.settings.impl

import com.warrenbrowse.vpn.lib.repository.WarrenSubscriptionOutcome
import com.warrenbrowse.vpn.lib.repository.WarrenVoucherOutcome
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import org.junit.jupiter.api.Test

class SubscriptionLabelTest {

    @Test
    fun `active subscription when expiry is in the future`() {
        val label = subscriptionLabel(
            WarrenSubscriptionOutcome.Success(expiresAtUnixSecs = 2_000),
            nowSecs = 1_000,
        )
        assertTrue(label.startsWith("Subscription active"), label)
    }

    @Test
    fun `expired subscription when expiry is in the past`() {
        val label = subscriptionLabel(
            WarrenSubscriptionOutcome.Success(expiresAtUnixSecs = 1_000),
            nowSecs = 2_000,
        )
        assertTrue(label.startsWith("Subscription expired"), label)
    }

    @Test
    fun `voucher success renders the new expiry, failures are fixed`() {
        assertTrue(
            voucherLabel(WarrenVoucherOutcome.Success(2_000)).startsWith("Voucher redeemed"),
        )
        assertEquals(
            "Authorization cancelled.",
            voucherLabel(WarrenVoucherOutcome.AuthorizationDenied),
        )
        assertEquals(
            "Couldn't redeem voucher. Check the code and try again.",
            voucherLabel(WarrenVoucherOutcome.Failure("invalid")),
        )
    }

    @Test
    fun `non-success outcomes map to fixed messages`() {
        assertEquals(
            "Authorization cancelled.",
            subscriptionLabel(WarrenSubscriptionOutcome.AuthorizationDenied),
        )
        assertEquals(
            "Set up your wallet first.",
            subscriptionLabel(WarrenSubscriptionOutcome.WalletNotReady),
        )
        assertEquals(
            "Couldn't fetch subscription status.",
            subscriptionLabel(WarrenSubscriptionOutcome.Failure("boom 500")),
        )
    }
}
