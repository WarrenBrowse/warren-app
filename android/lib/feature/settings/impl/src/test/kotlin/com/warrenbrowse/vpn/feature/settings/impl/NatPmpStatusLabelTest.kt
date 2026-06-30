package com.warrenbrowse.vpn.feature.settings.impl

import android.content.Context
import com.warrenbrowse.vpn.lib.ui.resource.R
import io.mockk.MockKAnswerScope
import io.mockk.every
import io.mockk.mockk
import kotlin.test.assertEquals
import kotlin.test.assertNull
import org.junit.jupiter.api.Test

// getString(int, vararg Any) delivers its format args as a single Array in
// invocation.args[1] (mockk does not flatten varargs); fall back to flattened
// args defensively.
private fun MockKAnswerScope<String, *>.fmtArgs(): List<Any?> =
    (invocation.args.getOrNull(1) as? Array<*>)?.toList() ?: invocation.args.drop(1)

class NatPmpStatusLabelTest {

    // Resolve the NAT-PMP status resources to their English templates so this
    // pure JSON-parsing test stays JVM-only while still asserting the composed
    // label per state.
    private val context: Context = mockk {
        every { getString(R.string.tunnel_natpmp_status_idle) } returns "Status: idle (no active mapping)"
        every { getString(R.string.tunnel_natpmp_status_requesting) } returns "Status: requesting a port…"
        every { getString(R.string.tunnel_natpmp_status_mapped) } returns "Status: mapped"
        every { getString(R.string.tunnel_natpmp_status_rate_limited) } returns "Status: rate-limited"
        every { getString(R.string.tunnel_natpmp_status_failed) } returns "Status: failed"
        every { getString(R.string.tunnel_natpmp_status_failed_port_in_use) } returns
            " - this public port is already in use on this exit. Switch exit or change the port."
        every { getString(eq(R.string.tunnel_natpmp_status_mapped_port), any()) } answers
            { " - external port ${fmtArgs()[0]}" }
        every { getString(eq(R.string.tunnel_natpmp_status_mapped_lifetime), any()) } answers
            { " (lifetime ${fmtArgs()[0]}s)" }
        every { getString(eq(R.string.tunnel_natpmp_status_rate_limited_retry), any()) } answers
            { " - retry in ${fmtArgs()[0]}s" }
        every { getString(eq(R.string.tunnel_natpmp_status_failed_reason), any()) } answers
            { " - ${fmtArgs()[0]}" }
    }

    @Test
    fun `idle and empty render the idle label`() {
        assertEquals("Status: idle (no active mapping)", natPmpStatusLabel(context, """{"state":"idle"}"""))
        assertEquals("Status: idle (no active mapping)", natPmpStatusLabel(context, "{}"))
    }

    @Test
    fun `requesting renders progress`() {
        assertEquals("Status: requesting a port…", natPmpStatusLabel(context, """{"state":"requesting"}"""))
    }

    @Test
    fun `mapped renders port and lifetime`() {
        val json = """{"state":"mapped","external_port":51820,"lifetime_secs":3600}"""
        assertEquals("Status: mapped - external port 51820 (lifetime 3600s)", natPmpStatusLabel(context, json))
    }

    @Test
    fun `rate limited renders retry countdown`() {
        assertEquals(
            "Status: rate-limited - retry in 30s",
            natPmpStatusLabel(context, """{"state":"rate_limited","retry_after_secs":30}"""),
        )
    }

    @Test
    fun `failed renders the reason category`() {
        assertEquals(
            "Status: failed - Unreachable",
            natPmpStatusLabel(context, """{"state":"failed","reason":"Unreachable"}"""),
        )
    }

    @Test
    fun `failed with a port conflict renders a friendly actionable message`() {
        // A followed port taken by another client on a new exit must not
        // surface the raw enum name; show what happened and how to recover.
        assertEquals(
            "Status: failed - this public port is already in use on this exit. " +
                "Switch exit or change the port.",
            natPmpStatusLabel(context, """{"state":"failed","reason":"SuggestedPortInUse"}"""),
        )
    }

    @Test
    fun `jsonField extracts strings and numbers and returns null when absent`() {
        val json = """{"state":"mapped","external_port":51820,"reason":"x"}"""
        assertEquals("mapped", jsonField(json, "state"))
        assertEquals("51820", jsonField(json, "external_port"))
        assertEquals("x", jsonField(json, "reason"))
        assertNull(jsonField(json, "missing"))
    }
}
