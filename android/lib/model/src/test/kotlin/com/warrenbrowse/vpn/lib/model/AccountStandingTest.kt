package com.warrenbrowse.vpn.lib.model

import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNotEquals
import kotlin.test.assertNull
import kotlin.test.assertTrue
import org.junit.jupiter.api.Test

class AccountStandingTest {
    private fun strike(reference: String, port: Int = 51413) =
        AccountStrike(
            dayUnixSecs = 1_790_000_000,
            category = AbuseCategory.Copyright,
            exitCountry = "FI",
            port = port,
            caseReference = reference,
        )

    @Test
    fun `the port-forwarding ban token is read with its lapse`() {
        assertEquals(
            AuthFailedError.BannedPortForwarding(1_821_536_000),
            AuthFailedError.banOf(
                "[BANNED_PORT_FORWARDING] the API refused to issue credentials",
                1_821_536_000,
            ),
        )
    }

    @Test
    fun `the generic ban token is read`() {
        assertEquals(
            AuthFailedError.Banned(null),
            AuthFailedError.banOf("[BANNED] exit rejected the session", null),
        )
    }

    @Test
    fun `a reason without a ban token is not a ban`() {
        assertNull(AuthFailedError.banOf("subscription expired", null))
        assertNull(AuthFailedError.banOf("[EXPIRED_ACCOUNT] no subscription", null))
        assertNull(AuthFailedError.banOf("text [BANNED] later", null))
    }

    @Test
    fun `the latest strike carries its rank and the threshold`() {
        val standing =
            AccountStanding(
                strikes = listOf(strike("PF-1"), strike("PF-2")),
                threshold = 3,
                windowDays = 90,
                ban = null,
            )

        assertEquals(StrikeNotice(strike("PF-2"), ordinal = 2, threshold = 3), standing.latestStrike())
        assertNull(standing.copy(strikes = emptyList()).latestStrike())
    }

    @Test
    fun `a strike renders neither its case nor its port when printed`() {
        val printed = strike("PF-2026-0042", port = 51413).toString()

        assertFalse(printed.contains("PF-2026-0042"), printed)
        assertFalse(printed.contains("51413"), printed)
    }

    @Test
    fun `the dismissal key tells strikes apart and names no case`() {
        val key = strike("PF-2026-0042").dismissalKey

        assertTrue(key.startsWith("strike:"))
        assertFalse(key.contains("PF-2026-0042"))
        assertNotEquals(key, strike("PF-2026-0043").dismissalKey)
    }

    @Test
    fun `an unknown abuse category reads as other`() {
        assertEquals(AbuseCategory.MalwareC2, AbuseCategory.of("malware_c2"))
        assertEquals(AbuseCategory.Other, AbuseCategory.of("something_new"))
        assertEquals(AbuseCategory.Other, AbuseCategory.of(null))
    }
}
