package com.warrenbrowse.vpn.feature.home.impl.connect.connectioninfo

import kotlin.test.assertEquals
import org.junit.jupiter.api.Test

/** The desktop `formatAge` shape for the "Last" reconnect row. */
class ReconnectAgeTest {

    @Test
    fun `under a minute the age is whole seconds`() {
        assertEquals("0s", formatReconnectAge(999L))
        assertEquals("42s", formatReconnectAge(42_500L))
    }

    @Test
    fun `under an hour the age is minutes and seconds`() {
        assertEquals("1m 0s", formatReconnectAge(60_000L))
        assertEquals("12m 7s", formatReconnectAge(727_000L))
    }

    @Test
    fun `from an hour on the age is hours and minutes`() {
        assertEquals("1h 0m", formatReconnectAge(3_600_000L))
        assertEquals("3h 25m", formatReconnectAge(12_300_000L))
    }

    @Test
    fun `a clock that went backwards reads as now`() {
        assertEquals("0s", formatReconnectAge(-5_000L))
    }
}
