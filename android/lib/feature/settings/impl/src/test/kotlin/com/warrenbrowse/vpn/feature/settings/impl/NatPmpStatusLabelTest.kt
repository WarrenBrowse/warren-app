package com.warrenbrowse.vpn.feature.settings.impl

import kotlin.test.assertEquals
import kotlin.test.assertNull
import org.junit.jupiter.api.Test

class NatPmpStatusLabelTest {

    @Test
    fun `idle and empty render the idle label`() {
        assertEquals("Status: idle (no active mapping)", natPmpStatusLabel("""{"state":"idle"}"""))
        assertEquals("Status: idle (no active mapping)", natPmpStatusLabel("{}"))
    }

    @Test
    fun `requesting renders progress`() {
        assertEquals("Status: requesting a port…", natPmpStatusLabel("""{"state":"requesting"}"""))
    }

    @Test
    fun `mapped renders port and lifetime`() {
        val json = """{"state":"mapped","external_port":51820,"lifetime_secs":3600}"""
        assertEquals("Status: mapped — external port 51820 (lifetime 3600s)", natPmpStatusLabel(json))
    }

    @Test
    fun `rate limited renders retry countdown`() {
        assertEquals(
            "Status: rate-limited — retry in 30s",
            natPmpStatusLabel("""{"state":"rate_limited","retry_after_secs":30}"""),
        )
    }

    @Test
    fun `failed renders the reason category`() {
        assertEquals(
            "Status: failed — Unreachable",
            natPmpStatusLabel("""{"state":"failed","reason":"Unreachable"}"""),
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
