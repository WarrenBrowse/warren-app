package com.warrenbrowse.vpn.lib.model

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Test

class WarrenNetworkStatsTest {

    private val stats: WarrenNetworkStats =
        WarrenNetworkStatsParser.parse(
            requireNotNull(javaClass.getResource("/network-stats-v1.json")).readText()
        )!!

    private val live = stats.exits[0]
    private val quiet = stats.exits[1]

    @Test
    fun `a live exit's people count is a floor`() {
        assertEquals(PeopleCount.AtLeast(40), stats.peopleOn(live))
    }

    @Test
    fun `a live exit under one rounding step says fewer than the step`() {
        assertEquals(PeopleCount.Below(5), stats.peopleOn(live.copy(connected = 0)))
    }

    @Test
    fun `a quiet exit says fewer than the live threshold, whatever it carries`() {
        assertEquals(PeopleCount.Below(20), stats.peopleOn(quiet.copy(connected = 15)))
    }

    @Test
    fun `the fleet count is a floor while an exit is live`() {
        assertEquals(PeopleCount.AtLeast(57), stats.fleetPeople())
    }

    @Test
    fun `the fleet count is exact while no exit is live`() {
        val allQuiet = stats.copy(exits = listOf(quiet))

        assertEquals(PeopleCount.Exact(57), allQuiet.fleetPeople())
    }

    @Test
    fun `a floored fleet count under one step says fewer than the step`() {
        val few = stats.copy(users = stats.users.copy(connected = 0))

        assertEquals(PeopleCount.Below(5), few.fleetPeople())
    }

    @Test
    fun `an exit is looked up by id in any case`() {
        assertEquals(live, stats.exit("ABABABABABABABABABABABABABABABAB"))
        assertNull(stats.exit("ffffffffffffffffffffffffffffffff"))
    }

    @Test
    fun `an offline exit shows as offline even when it claims to be live`() {
        assertEquals(ExitDisplayMode.OFFLINE, live.copy(online = false).displayMode)
        assertEquals(ExitDisplayMode.LIVE, live.displayMode)
    }

    @Test
    fun `wire tokens map to bands and drivers`() {
        assertEquals(LoadLevel.SATURATED, LoadLevel.of("saturated"))
        assertEquals(LoadLevel.HIGH, LoadLevel.of("high"))
        assertEquals(LoadLevel.UNKNOWN, LoadLevel.of(null))
        assertEquals(LoadDriver.CPU, LoadDriver.of("cpu"))
        assertNull(LoadDriver.of(null))
    }
}
