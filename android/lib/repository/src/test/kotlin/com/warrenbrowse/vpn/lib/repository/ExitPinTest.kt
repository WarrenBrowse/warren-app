package com.warrenbrowse.vpn.lib.repository

import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue
import org.junit.jupiter.api.Test

/**
 * Resolution of a location-picker pin down to one concrete exit. The picker
 * lets the user pin any geographical depth (country, city, single exit) but the
 * engine only ever accepts one exit, so this is the rule that bridges the two.
 */
class ExitPinTest {

    private fun relay(
        exitId: String,
        country: String = "DE",
        city: String = "Frankfurt",
        active: Boolean = true,
        weight: Long = 1,
    ) = WarrenRelaySummary(
        exitId = exitId,
        exitPubkeyHex = "aa",
        endpoint = "10.0.0.1:443",
        country = country,
        city = city,
        active = active,
        weight = weight,
    )

    private val catalogue = listOf(
        relay("de1", country = "DE", city = "Frankfurt", weight = 10),
        relay("de2", country = "DE", city = "Frankfurt", weight = 30),
        relay("de3", country = "DE", city = "Berlin", weight = 50),
        relay("fr1", country = "FR", city = "Paris", weight = 90),
        relay("fr2", country = "FR", city = "Paris", active = false, weight = 99),
    )

    @Test
    fun `automatic pins nothing so the caller keeps its own fallback`() {
        assertNull(resolveExitPin(ExitPin.Automatic, catalogue))
    }

    @Test
    fun `an exit pin resolves to that exit`() {
        assertEquals("de1", resolveExitPin(ExitPin.Exit("de1"), catalogue)?.exitId)
    }

    @Test
    fun `an exit pin on an inactive exit resolves to nothing`() {
        assertNull(resolveExitPin(ExitPin.Exit("fr2"), catalogue))
    }

    @Test
    fun `an exit pin on an unknown exit resolves to nothing`() {
        assertNull(resolveExitPin(ExitPin.Exit("zz9"), catalogue))
    }

    @Test
    fun `a country pin resolves to the heaviest active exit in that country`() {
        assertEquals("de3", resolveExitPin(ExitPin.Country("DE"), catalogue)?.exitId)
    }

    @Test
    fun `a country pin never resolves to an inactive exit`() {
        assertEquals("fr1", resolveExitPin(ExitPin.Country("FR"), catalogue)?.exitId)
    }

    @Test
    fun `a country pin matches the catalogue case-insensitively`() {
        assertEquals("de3", resolveExitPin(ExitPin.Country("de"), catalogue)?.exitId)
    }

    @Test
    fun `a country pin with no active exit resolves to nothing`() {
        val down = listOf(relay("se1", country = "SE", active = false))
        assertNull(resolveExitPin(ExitPin.Country("SE"), down))
    }

    @Test
    fun `a city pin resolves inside that city only`() {
        assertEquals("de2", resolveExitPin(ExitPin.City("DE", "Frankfurt"), catalogue)?.exitId)
    }

    @Test
    fun `a city pin matches the catalogue case-insensitively`() {
        assertEquals("de2", resolveExitPin(ExitPin.City("de", "frankfurt"), catalogue)?.exitId)
    }

    @Test
    fun `a city pin with no active exit resolves to nothing`() {
        assertNull(resolveExitPin(ExitPin.City("FR", "Paris"), listOf(catalogue[4])))
    }

    @Test
    fun `equal weights resolve to the same exit on every dial`() {
        val tied = listOf(
            relay("b", country = "NL", weight = 7),
            relay("a", country = "NL", weight = 7),
        )
        assertEquals("b", resolveExitPin(ExitPin.Country("NL"), tied)?.exitId)
        assertEquals("b", resolveExitPin(ExitPin.Country("NL"), tied.reversed())?.exitId)
    }

    // Failover: the exit an automatic retry dials once the one it was on
    // dropped. The rule is the desktop selector's (same country first, then any
    // country in scope, never the failed exit itself, nothing outside the pin).

    private fun failoverAfter(pin: ExitPin, failed: String, relays: List<WarrenRelaySummary> = catalogue) =
        resolveFailoverExit(pin, exitCountry = null, relays = relays, failedExitPubkeyHex = failed)

    private val pubkeyed = listOf(
        relay("de1", country = "DE", city = "Frankfurt", weight = 10).copy(exitPubkeyHex = "k-de1"),
        relay("de2", country = "DE", city = "Frankfurt", weight = 30).copy(exitPubkeyHex = "k-de2"),
        relay("de3", country = "DE", city = "Berlin", weight = 50).copy(exitPubkeyHex = "k-de3"),
        relay("fr1", country = "FR", city = "Paris", weight = 90).copy(exitPubkeyHex = "k-fr1"),
        relay("nl1", country = "NL", city = "Amsterdam", weight = 20).copy(exitPubkeyHex = "k-nl1"),
    )

    @Test
    fun `a failover prefers an alternative in the failed exit's country`() {
        // fr1 is heavier, but the failed exit was German and Germany has spares.
        assertEquals("de2", failoverAfter(ExitPin.Automatic, "k-de3", pubkeyed)?.exitId)
    }

    @Test
    fun `a failover never returns the exit that failed`() {
        val only = listOf(pubkeyed[3])
        assertNull(failoverAfter(ExitPin.Automatic, "k-fr1", only))
    }

    @Test
    fun `a failover falls back to any country when the failed country has no spare`() {
        assertEquals("de3", failoverAfter(ExitPin.Automatic, "k-fr1", pubkeyed)?.exitId)
    }

    @Test
    fun `a failover stays inside a country pin`() {
        // Germany pinned, the German exits all failed except none: the French
        // one is heavier but out of scope.
        val germanyDown = pubkeyed.map { if (it.country == "DE" && it.exitId != "de1") it.copy(active = false) else it }
        assertEquals("de1", failoverAfter(ExitPin.Country("DE"), "k-de3", germanyDown)?.exitId)
        assertNull(failoverAfter(ExitPin.Country("DE"), "k-de1", germanyDown))
    }

    @Test
    fun `a failover stays inside a city pin`() {
        assertEquals("de1", failoverAfter(ExitPin.City("DE", "Frankfurt"), "k-de2", pubkeyed)?.exitId)
        assertNull(failoverAfter(ExitPin.City("DE", "Berlin"), "k-de3", pubkeyed))
    }

    @Test
    fun `a single exit pin leaves no failover alternative`() {
        assertNull(failoverAfter(ExitPin.Exit("de3"), "k-de3", pubkeyed))
    }

    @Test
    fun `an automatic pin with a preferred country fails over inside that country`() {
        val picked =
            resolveFailoverExit(ExitPin.Automatic, exitCountry = "DE", relays = pubkeyed, failedExitPubkeyHex = "k-de3")
        assertEquals("de2", picked?.exitId)
        assertNull(resolveFailoverExit(ExitPin.Automatic, exitCountry = "FR", relays = pubkeyed, failedExitPubkeyHex = "k-fr1"))
    }

    @Test
    fun `a failover from an exit the catalogue no longer lists still picks in scope`() {
        assertEquals("fr1", failoverAfter(ExitPin.Automatic, "k-gone", pubkeyed)?.exitId)
    }

    @Test
    fun `a country pin marks its country row and nothing deeper`() {
        assertTrue(ExitPin.Country("DE").pinsCountry("de"))
        assertFalse(ExitPin.Country("DE").pinsCountry("FR"))
        assertFalse(ExitPin.City("DE", "Berlin").pinsCountry("DE"))
        assertFalse(ExitPin.Automatic.pinsCountry("DE"))
    }

    @Test
    fun `a city pin marks its city row and nothing deeper`() {
        assertTrue(ExitPin.City("DE", "Berlin").pinsCity("de", "berlin"))
        assertFalse(ExitPin.City("DE", "Berlin").pinsCity("DE", "Frankfurt"))
        assertFalse(ExitPin.Country("DE").pinsCity("DE", "Berlin"))
        assertFalse(ExitPin.Automatic.pinsCity("DE", "Berlin"))
    }

    // Failover: the retry after a drop must not redial the exit that just
    // failed when the pin leaves it a choice (desktop
    // `select_failover_alternative`: same country first, then anywhere the
    // pin allows, never the excluded exit, and never wider than the pin).

    private fun pubkeyOf(exitId: String) = "pk-$exitId"

    private val keyed = catalogue.map { it.copy(exitPubkeyHex = pubkeyOf(it.exitId)) }

    private fun failover(pin: ExitPin, failed: String, exitCountry: String? = null) =
        resolveFailoverExit(pin, exitCountry, keyed, pubkeyOf(failed))?.exitId

    @Test
    fun `a failover under automatic prefers another exit in the same country`() {
        assertEquals("de3", failover(ExitPin.Automatic, failed = "de2"))
    }

    @Test
    fun `a failover under automatic leaves the country when it has no other exit`() {
        assertEquals("de3", failover(ExitPin.Automatic, failed = "fr1"))
    }

    @Test
    fun `a failover under automatic stays inside the preferred exit country`() {
        assertNull(failover(ExitPin.Automatic, failed = "fr1", exitCountry = "FR"))
        assertEquals("de3", failover(ExitPin.Automatic, failed = "de2", exitCountry = "DE"))
    }

    @Test
    fun `a failover never returns the failed exit`() {
        val only = listOf(keyed[0])
        assertNull(resolveFailoverExit(ExitPin.Automatic, null, only, pubkeyOf("de1")))
    }

    @Test
    fun `a failover under a country pin stays inside that country`() {
        assertNull(failover(ExitPin.Country("FR"), failed = "fr1"))
        assertEquals("de3", failover(ExitPin.Country("DE"), failed = "de2"))
    }

    @Test
    fun `a failover under a city pin stays inside that city`() {
        assertEquals("de1", failover(ExitPin.City("DE", "Frankfurt"), failed = "de2"))
        assertNull(failover(ExitPin.City("DE", "Berlin"), failed = "de3"))
    }

    @Test
    fun `a failover under a single exit pin has no alternative`() {
        assertNull(failover(ExitPin.Exit("de2"), failed = "de2"))
    }

    @Test
    fun `a failover skips inactive exits`() {
        // fr2 is heavier than fr1 and inactive.
        assertNull(failover(ExitPin.Country("FR"), failed = "fr1"))
    }

    @Test
    fun `a failover from an exit the catalogue no longer knows picks inside the pin`() {
        assertEquals("fr1", failover(ExitPin.Automatic, failed = "gone"))
    }
}
