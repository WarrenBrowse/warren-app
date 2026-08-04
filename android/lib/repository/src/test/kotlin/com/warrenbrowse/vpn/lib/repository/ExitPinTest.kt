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
}
