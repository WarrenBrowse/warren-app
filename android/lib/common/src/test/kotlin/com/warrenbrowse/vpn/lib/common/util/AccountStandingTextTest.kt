package com.warrenbrowse.vpn.lib.common.util

import android.content.Context
import com.warrenbrowse.vpn.lib.model.AbuseCategory
import com.warrenbrowse.vpn.lib.model.AccountBan
import com.warrenbrowse.vpn.lib.model.AccountStrike
import com.warrenbrowse.vpn.lib.model.StrikeNotice
import com.warrenbrowse.vpn.lib.ui.resource.R
import io.mockk.every
import io.mockk.mockk
import java.util.Locale
import kotlin.test.assertEquals
import org.junit.jupiter.api.Test

class AccountStandingTextTest {
    // The English templates, with the arguments spliced in by position.
    private val context: Context = mockk {
        every { getString(R.string.abuse_category_copyright) } returns "copyright"
        every { getString(R.string.account_ban_port_forwarding) } returns
            "Access suspended for port-forwarding abuse."
        every { getString(R.string.account_ban) } returns "Access suspended."
        every { getString(eq(R.string.account_ban_port_forwarding_until), any()) } answers
            { "Access suspended for port-forwarding abuse until ${args(this)[0]}." }
        every { getString(eq(R.string.account_ban_until), any()) } answers
            { "Access suspended until ${args(this)[0]}." }
        every { getString(eq(R.string.account_strike_warning), *anyVararg()) } answers
            {
                val a = args(this)
                "Warning ${a[0]} of ${a[1]}: port ${a[2]} was closed on ${a[3]} after an abuse report (${a[4]})."
            }
        every { getString(eq(R.string.account_strike_warning_no_threshold), *anyVararg()) } answers
            {
                val a = args(this)
                "Warning ${a[0]}: port ${a[1]} was closed on ${a[2]} after an abuse report (${a[3]})."
            }
    }

    private fun args(scope: io.mockk.MockKAnswerScope<String, *>): List<Any?> =
        (scope.invocation.args.getOrNull(1) as? Array<*>)?.toList()
            ?: scope.invocation.args.drop(1)

    private val strike =
        AccountStrike(
            // 2026-09-24T00:00:00Z, the day the API writes a strike as.
            dayUnixSecs = 1_790_208_000,
            category = AbuseCategory.Copyright,
            exitCountry = "FI",
            port = 51413,
            caseReference = "PF-2026-0042",
        )

    @Test
    fun `a strike day is the UTC day it was recorded on, wherever the reader is`() {
        val previous = java.util.TimeZone.getDefault()
        java.util.TimeZone.setDefault(java.util.TimeZone.getTimeZone("America/Los_Angeles"))
        try {
            assertEquals("September 24, 2026", AccountStandingText.day(1_790_208_000, Locale.US))
        } finally {
            java.util.TimeZone.setDefault(previous)
        }
    }

    @Test
    fun `a strike warns with its rank, the threshold, the port, the day and the category`() {
        assertEquals(
            "Warning 2 of 3: port 51413 was closed on September 24, 2026 after an abuse report (copyright).",
            AccountStandingText.warning(context, StrikeNotice(strike, 2, 3), Locale.US),
        )
    }

    @Test
    fun `a strike whose threshold is unknown does not invent one`() {
        assertEquals(
            "Warning 1: port 51413 was closed on September 24, 2026 after an abuse report (copyright).",
            AccountStandingText.warning(context, StrikeNotice(strike, 1, 0), Locale.US),
        )
    }

    @Test
    fun `a port-forwarding ban is dated by its lapse when it is known`() {
        val ban = AccountBan(portForwarding = true, lapsesAtUnixSecs = 1_821_744_000, inForce = true)

        assertEquals(
            "Access suspended for port-forwarding abuse until September 24, 2027.",
            AccountStandingText.ban(context, ban, Locale.US),
        )
        assertEquals(
            "Access suspended for port-forwarding abuse.",
            AccountStandingText.ban(context, ban.copy(lapsesAtUnixSecs = null), Locale.US),
        )
    }

    @Test
    fun `any other ban says only that access is suspended`() {
        val ban = AccountBan(portForwarding = false, lapsesAtUnixSecs = null, inForce = true)

        assertEquals("Access suspended.", AccountStandingText.ban(context, ban, Locale.US))
    }
}
