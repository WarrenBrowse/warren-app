package com.warrenbrowse.vpn.feature.home.impl.connect

import android.content.Context
import com.warrenbrowse.vpn.lib.ui.resource.R
import io.mockk.every
import io.mockk.mockk
import kotlin.test.assertEquals
import kotlin.test.assertNull
import kotlin.test.assertTrue
import org.junit.jupiter.api.Test

class ConnectExpiryWarningTest {

    private val now = 1_000_000L

    // Resolve the warning string resources to their English templates so this
    // pure date-logic test stays JVM-only while still asserting branch + day
    // count selection.
    private val context: Context = mockk {
        every { getString(R.string.connect_subscription_expired) } returns
            "Your subscription has expired. Tap to renew."
        every { getString(eq(R.string.connect_subscription_expires_in_day), any()) } answers
            { "Your subscription expires in ${arg<Any>(1)} day. Tap to renew." }
        every { getString(eq(R.string.connect_subscription_expires_in_days), any()) } answers
            { "Your subscription expires in ${arg<Any>(1)} days. Tap to renew." }
    }

    @Test
    fun `no warning when expiry unknown`() {
        assertNull(connectExpiryWarning(context, 0L, nowSecs = now))
    }

    @Test
    fun `no warning when comfortably active`() {
        assertNull(connectExpiryWarning(context, now + 30 * 86_400, nowSecs = now))
    }

    @Test
    fun `warns within a week with a day count`() {
        val label = connectExpiryWarning(context, now + 2 * 86_400, nowSecs = now)
        assertTrue(label != null && label.startsWith("Your subscription expires in 2 days"), "$label")
    }

    @Test
    fun `singular day phrasing at one day`() {
        val label = connectExpiryWarning(context, now + 86_400, nowSecs = now)
        assertTrue(label != null && label.contains("in 1 day."), "$label")
    }

    @Test
    fun `expired warning when past`() {
        assertEquals(
            "Your subscription has expired. Tap to renew.",
            connectExpiryWarning(context, now - 86_400, nowSecs = now),
        )
    }
}
