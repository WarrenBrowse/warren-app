package com.warrenbrowse.vpn.feature.home.impl.connect

import android.content.Context
import com.warrenbrowse.vpn.lib.ui.resource.R
import io.mockk.MockKAnswerScope
import io.mockk.every
import io.mockk.mockk
import kotlin.test.assertEquals
import kotlin.test.assertNull
import kotlin.test.assertTrue
import org.junit.jupiter.api.Test

// getString(int, vararg Any) delivers its format args as a single Array in
// invocation.args[1] (mockk does not flatten varargs); fall back to flattened
// args defensively.
private fun MockKAnswerScope<String, *>.fmtArgs(): List<Any?> =
    (invocation.args.getOrNull(1) as? Array<*>)?.toList() ?: invocation.args.drop(1)

class ConnectExpiryWarningTest {

    private val now = 1_000_000L

    // Resolve the warning string resources to their English templates so this
    // pure date-logic test stays JVM-only while still asserting branch + day
    // count selection.
    private val context: Context = mockk {
        every { getString(R.string.connect_subscription_expired) } returns
            "Your account credit has expired. Buy more credit."
        every { getString(eq(R.string.connect_subscription_expires_in_day), any()) } answers
            { "${fmtArgs()[0]} day left. Buy more credit." }
        every { getString(eq(R.string.connect_subscription_expires_in_days), any()) } answers
            { "${fmtArgs()[0]} days left. Buy more credit." }
        every { resources } returns mockk {
            every {
                getQuantityString(eq(R.plurals.home_days_left), any(), any())
            } answers { "${(invocation.args[2] as Array<*>)[0]} days left" }
            every {
                getQuantityString(eq(R.plurals.home_months_left), any(), any())
            } answers { "${(invocation.args[2] as Array<*>)[0]} months left" }
            every {
                getQuantityString(eq(R.plurals.home_years_left), any(), any())
            } answers { "${(invocation.args[2] as Array<*>)[0]} years left" }
        }
    }

    @Test
    fun `no time-left footer when expiry unknown`() {
        assertNull(accountTimeLeftLabel(context, 0L, nowSecs = now))
    }

    @Test
    fun `no time-left footer within the last week (banner covers it)`() {
        assertNull(accountTimeLeftLabel(context, now + 3 * 86_400, nowSecs = now))
    }

    @Test
    fun `time-left footer counts plain days under a month`() {
        assertEquals(
            "12 days left",
            accountTimeLeftLabel(context, now + 12 * 86_400, nowSecs = now),
        )
    }

    @Test
    fun `time-left footer rolls a month up from thirty days`() {
        assertEquals(
            "1 months left",
            accountTimeLeftLabel(context, now + 30 * 86_400, nowSecs = now),
        )
    }

    @Test
    fun `time-left footer names a year instead of four hundred days`() {
        assertEquals(
            "1 years left",
            accountTimeLeftLabel(context, now + 400 * 86_400, nowSecs = now),
        )
    }

    @Test
    fun `time-left footer floors to whole days, like the account card`() {
        // The header and the account card must never name different units for
        // one expiry, so both floor and both coarsen at 30 and 365 days.
        assertEquals(
            "12 months left",
            accountTimeLeftLabel(context, now + 364 * 86_400 + 43_200, nowSecs = now),
        )
        assertEquals(
            "12 days left",
            accountTimeLeftLabel(context, now + 12 * 86_400 + 43_200, nowSecs = now),
        )
    }
}
