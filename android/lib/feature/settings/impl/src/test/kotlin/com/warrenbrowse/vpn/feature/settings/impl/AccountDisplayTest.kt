package com.warrenbrowse.vpn.feature.settings.impl

import java.time.ZoneOffset
import java.util.Locale
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import org.junit.jupiter.api.Test

class AccountDisplayTest {

    @Test
    fun `paid until is unavailable when the expiry is unknown`() {
        // 0 is both "never fetched" and the 404 no-subscription sentinel; both
        // must read as unavailable, never as expired-in-1970.
        assertEquals(PaidUntilDisplay.Unavailable, paidUntilDisplay(0L, nowSecs = 1_000))
        assertEquals(PaidUntilDisplay.Unavailable, paidUntilDisplay(-5L, nowSecs = 1_000))
    }

    @Test
    fun `paid until is out of time when the expiry is in the past`() {
        assertEquals(PaidUntilDisplay.OutOfTime, paidUntilDisplay(999L, nowSecs = 1_000))
        assertEquals(PaidUntilDisplay.OutOfTime, paidUntilDisplay(1_000L, nowSecs = 1_000))
    }

    @Test
    fun `paid until shows the date when the expiry is in the future`() {
        assertEquals(
            PaidUntilDisplay.Date(2_000L),
            paidUntilDisplay(2_000L, nowSecs = 1_000),
        )
    }

    @Test
    fun `remaining time is none when expired or unknown`() {
        assertEquals(RemainingTime.None, remainingTime(0L, nowSecs = 1_000))
        assertEquals(RemainingTime.None, remainingTime(999L, nowSecs = 1_000))
    }

    @Test
    fun `remaining time floors to whole days`() {
        val now = 1_000_000L
        assertEquals(
            RemainingTime.Days(26),
            remainingTime(now + 26 * 86_400 + 3_600, nowSecs = now),
        )
    }

    @Test
    fun `remaining time under a day reads as less than a day`() {
        val now = 1_000_000L
        assertEquals(RemainingTime.LessThanADay, remainingTime(now + 86_399, nowSecs = now))
    }

    @Test
    fun `remaining time from a year up reads in years`() {
        val now = 1_000_000L
        assertEquals(
            RemainingTime.Years(2),
            remainingTime(now + 731 * 86_400, nowSecs = now),
        )
        assertEquals(
            RemainingTime.Years(1),
            remainingTime(now + 365 * 86_400, nowSecs = now),
        )
    }

    @Test
    fun `remaining time from a month up reads in months`() {
        // The home header and the account card must name the same unit for the
        // same expiry, so both coarsen at 30 days and again at 365.
        val now = 1_000_000L
        assertEquals(
            RemainingTime.Months(1),
            remainingTime(now + 30 * 86_400, nowSecs = now),
        )
        assertEquals(
            RemainingTime.Months(12),
            remainingTime(now + 364 * 86_400, nowSecs = now),
        )
        assertEquals(
            RemainingTime.Days(29),
            remainingTime(now + 29 * 86_400, nowSecs = now),
        )
    }

    @Test
    fun `expiry date renders medium date plus short time`() {
        // 2026-03-01T12:30:00Z rendered in UTC / English.
        val formatted = formatExpiryDateTime(1_772_368_200L, ZoneOffset.UTC, Locale.ENGLISH)
        assertTrue(formatted.contains("2026"), formatted)
        assertTrue(formatted.contains("12:30"), formatted)
    }

    @Test
    fun `voucher filter uppercases, drops non-crockford chars and caps at 16`() {
        // Dashes, spaces and the ambiguous I/L/O/U are dropped, not aliased.
        assertEquals("ABCD1234EFGH5678", filterVoucherInput("abcd-1234 efgh-5678"))
        assertEquals("AB12", filterVoucherInput("aIbL1oO2uU"))
        assertEquals(16, filterVoucherInput("ABCD1234EFGH5678XYZ9").length)
    }

    @Test
    fun `voucher completeness requires exactly 16 crockford chars`() {
        assertTrue(isCompleteVoucher("ABCD1234EFGH5678"))
        assertTrue(!isCompleteVoucher("ABCD1234EFGH567"))
        assertTrue(!isCompleteVoucher("ABCD1234EFGH567I"))
    }

    @Test
    fun `voucher grouping renders 4-4-4-4 and maps cursor offsets both ways`() {
        val transformed = VoucherVisualTransformation.filter(
            androidx.compose.ui.text.AnnotatedString("ABCD1234EFGH5678"),
        )
        assertEquals("ABCD-1234-EFGH-5678", transformed.text.text)

        val mapping = transformed.offsetMapping
        assertEquals(0, mapping.originalToTransformed(0))
        assertEquals(4, mapping.originalToTransformed(4))
        assertEquals(6, mapping.originalToTransformed(5))
        assertEquals(19, mapping.originalToTransformed(16))
        assertEquals(4, mapping.transformedToOriginal(4))
        assertEquals(4, mapping.transformedToOriginal(5))
        assertEquals(8, mapping.transformedToOriginal(9))
        assertEquals(16, mapping.transformedToOriginal(19))
    }
}
