package com.warrenbrowse.vpn.feature.settings.impl

import com.warrenbrowse.vpn.lib.repository.ExitPin
import com.warrenbrowse.vpn.lib.repository.WarrenConnectedInfo
import com.warrenbrowse.vpn.lib.repository.WarrenRelaySummary
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue
import org.junit.jupiter.api.Test

/**
 * The location picker's decision logic, kept out of the composable so the
 * desktop-parity rules (a pick is terminal, a pick connects, an entry pick
 * flips to the exit hop) are pinned by tests rather than by inspection.
 */
class LocationPickerSelectionTest {

    private fun relay(exitId: String, country: String, city: String) = WarrenRelaySummary(
        exitId = exitId,
        exitPubkeyHex = "aa",
        endpoint = "10.0.0.1:443",
        country = country,
        city = city,
        active = true,
        weight = 1,
    )

    private val catalogue = listOf(
        relay("de1", "DE", "Frankfurt"),
        relay("de2", "DE", "Berlin"),
        relay("fr1", "FR", "Paris"),
    )

    @Test
    fun `a pick reconnects the live tunnel`() {
        val connected = WarrenConnectedInfo.Connected(
            exitEndpointHost = "10.0.0.1:443",
            entryEndpointHost = null,
            multiHop = false,
            daita = false,
            assignedNatPmpPort = null,
        )
        assertEquals(PickFollowUp.Reconnect, pickFollowUp(connected))
    }

    @Test
    fun `a pick connects while disconnected`() {
        assertEquals(PickFollowUp.Connect, pickFollowUp(WarrenConnectedInfo.Disconnected))
    }

    @Test
    fun `a pick connects after a failure`() {
        assertEquals(PickFollowUp.Connect, pickFollowUp(WarrenConnectedInfo.Failed("boom")))
    }

    @Test
    fun `a pick connects while the kill switch is blocking`() {
        assertEquals(PickFollowUp.Connect, pickFollowUp(WarrenConnectedInfo.Blocking("blocked")))
    }

    @Test
    fun `a pick queues nothing while a transition is already in flight`() {
        assertEquals(PickFollowUp.None, pickFollowUp(WarrenConnectedInfo.Connecting()))
        assertEquals(PickFollowUp.None, pickFollowUp(WarrenConnectedInfo.Reconnecting()))
        assertEquals(PickFollowUp.None, pickFollowUp(WarrenConnectedInfo.Disconnecting()))
    }

    @Test
    fun `rows are inert only while a transition is in flight`() {
        assertTrue(transitionInFlight(WarrenConnectedInfo.Connecting()))
        assertTrue(transitionInFlight(WarrenConnectedInfo.Reconnecting()))
        assertTrue(transitionInFlight(WarrenConnectedInfo.Disconnecting()))
        assertFalse(transitionInFlight(WarrenConnectedInfo.Disconnected))
        assertFalse(transitionInFlight(WarrenConnectedInfo.Failed("boom")))
    }

    @Test
    fun `picking an entry country writes it and flips to the exit hop`() {
        var written: String? = "SE"
        var wrote = false

        val next = applyEntryPick("FR") { code ->
            written = code
            wrote = true
        }

        assertTrue(wrote)
        assertEquals("FR", written)
        assertEquals(PickerScope.Exit, next)
    }

    @Test
    fun `picking automatic entry clears the country and flips to the exit hop`() {
        var written: String? = "SE"

        val next = applyEntryPick(null) { code -> written = code }

        assertEquals(null, written)
        assertEquals(PickerScope.Exit, next)
    }

    @Test
    fun `an exit pin expands the country and city holding it`() {
        val keys = expandedKeysFor(ExitPin.Exit("de1"), catalogue)

        assertEquals(setOf("DE"), keys.countries)
        assertEquals(setOf(cityKey("DE", "Frankfurt")), keys.cities)
    }

    @Test
    fun `a city pin expands its country and itself`() {
        val keys = expandedKeysFor(ExitPin.City("DE", "Berlin"), catalogue)

        assertEquals(setOf("DE"), keys.countries)
        assertEquals(setOf(cityKey("DE", "Berlin")), keys.cities)
    }

    @Test
    fun `a country pin expands the country only`() {
        val keys = expandedKeysFor(ExitPin.Country("FR"), catalogue)

        assertEquals(setOf("FR"), keys.countries)
        assertTrue(keys.cities.isEmpty())
    }

    @Test
    fun `automatic expands nothing`() {
        val keys = expandedKeysFor(ExitPin.Automatic, catalogue)

        assertTrue(keys.countries.isEmpty())
        assertTrue(keys.cities.isEmpty())
    }

    @Test
    fun `an exit pin naming an exit absent from the catalogue expands nothing`() {
        val keys = expandedKeysFor(ExitPin.Exit("zz9"), catalogue)

        assertTrue(keys.countries.isEmpty())
        assertTrue(keys.cities.isEmpty())
    }
}
