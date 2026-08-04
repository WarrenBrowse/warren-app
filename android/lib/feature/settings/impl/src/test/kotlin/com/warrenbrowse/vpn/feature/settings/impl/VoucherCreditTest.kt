package com.warrenbrowse.vpn.feature.settings.impl

import kotlin.test.assertEquals
import org.junit.jupiter.api.Test

/**
 * The credited duration shown in the voucher success dialog is derived from the
 * expiry the account carried before the redeem, so the dialog can say how much
 * time the voucher added and not only when the account now lapses.
 */
class VoucherCreditTest {

    @Test
    fun `a voucher on an account that never paid credits from now`() {
        assertEquals(
            RemainingTime.Months(1),
            creditedTime(
                previousExpiryUnixSecs = 0,
                newExpiryUnixSecs = NOW + 30 * DAY,
                nowSecs = NOW,
            ),
        )
    }

    @Test
    fun `a voucher on a lapsed account credits from now, not from the stale expiry`() {
        assertEquals(
            RemainingTime.Months(1),
            creditedTime(
                previousExpiryUnixSecs = NOW - 100 * DAY,
                newExpiryUnixSecs = NOW + 30 * DAY,
                nowSecs = NOW,
            ),
        )
    }

    @Test
    fun `a renewal credits from the previous expiry, not from now`() {
        assertEquals(
            RemainingTime.Months(1),
            creditedTime(
                previousExpiryUnixSecs = NOW + 10 * DAY,
                newExpiryUnixSecs = NOW + 40 * DAY,
                nowSecs = NOW,
            ),
        )
    }

    @Test
    fun `a multi-year voucher credits in years`() {
        assertEquals(
            RemainingTime.Years(2),
            creditedTime(
                previousExpiryUnixSecs = 0,
                newExpiryUnixSecs = NOW + 730 * DAY,
                nowSecs = NOW,
            ),
        )
    }

    @Test
    fun `an expiry that did not move credits nothing`() {
        assertEquals(
            RemainingTime.None,
            creditedTime(
                previousExpiryUnixSecs = NOW + 10 * DAY,
                newExpiryUnixSecs = NOW + 10 * DAY,
                nowSecs = NOW,
            ),
        )
    }

    @Test
    fun `a credit under a day is reported as less than a day`() {
        assertEquals(
            RemainingTime.LessThanADay,
            creditedTime(previousExpiryUnixSecs = 0, newExpiryUnixSecs = NOW + 3600, nowSecs = NOW),
        )
    }

    private companion object {
        const val DAY = 86_400L
        const val NOW = 1_800_000_000L
    }
}
