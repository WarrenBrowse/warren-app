package com.warrenbrowse.vpn.feature.home.impl.connect

import kotlin.test.assertEquals
import kotlin.test.assertNull
import kotlin.test.assertTrue
import org.junit.jupiter.api.Test

class ConnectExpiryWarningTest {

    private val now = 1_000_000L

    @Test
    fun `no warning when expiry unknown`() {
        assertNull(connectExpiryWarning(0L, nowSecs = now))
    }

    @Test
    fun `no warning when comfortably active`() {
        assertNull(connectExpiryWarning(now + 30 * 86_400, nowSecs = now))
    }

    @Test
    fun `warns within a week with a day count`() {
        val label = connectExpiryWarning(now + 2 * 86_400, nowSecs = now)
        assertTrue(label != null && label.startsWith("Your subscription expires in 2 days"), "$label")
    }

    @Test
    fun `singular day phrasing at one day`() {
        val label = connectExpiryWarning(now + 86_400, nowSecs = now)
        assertTrue(label != null && label.contains("in 1 day."), "$label")
    }

    @Test
    fun `expired warning when past`() {
        assertEquals(
            "Your subscription has expired. Tap to renew.",
            connectExpiryWarning(now - 86_400, nowSecs = now),
        )
    }
}
