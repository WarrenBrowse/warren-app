package com.warrenbrowse.vpn.app.service

import kotlin.test.assertEquals
import org.junit.jupiter.api.Test

class BanVerdictTest {
    @Test
    fun `the verdict keeps the ban token and the lapse`() {
        assertEquals(
            BanVerdict("[BANNED_PORT_FORWARDING] access suspended", 1_821_536_000),
            BanVerdict.parse(
                """{"reason":"[BANNED_PORT_FORWARDING] access suspended","lapses_at_unix_secs":1821536000}"""
            ),
        )
        assertEquals(
            BanVerdict("[BANNED] access suspended", null),
            BanVerdict.parse("""{"reason":"[BANNED] access suspended","lapses_at_unix_secs":null}"""),
        )
    }

    @Test
    fun `a verdict that names no ban is still the generic suspension`() {
        // The status edge already said the wallet is banned: an empty or
        // unreadable verdict must not turn that into something else.
        assertEquals(BanVerdict.UNKNOWN, BanVerdict.parse("{}"))
        assertEquals(BanVerdict.UNKNOWN, BanVerdict.parse("garbage"))
        assertEquals(BanVerdict.UNKNOWN, BanVerdict.parse("""{"reason":"subscription expired"}"""))
    }
}
