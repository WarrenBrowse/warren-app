package com.warrenbrowse.vpn.feature.home.impl.connect

import com.warrenbrowse.vpn.lib.repository.ExitPin
import com.warrenbrowse.vpn.lib.repository.WarrenRelaySummary
import kotlin.test.assertEquals
import kotlin.test.assertNull
import kotlin.test.assertTrue
import org.junit.jupiter.api.Test

class ShufflePickerTest {

    private val de = relay("exit-de", "185.65.135.10:443", "de", active = true)
    private val fi = relay("exit-fi", "146.70.1.2:443", "fi", active = true)
    private val nl = relay("exit-nl", "5.6.7.8:443", "nl", active = false)
    private val relays = listOf(de, fi, nl)

    @Test
    fun `shuffle offers only active exits`() {
        val candidates = shuffleCandidates(relays, currentExitId = null)
        assertTrue(candidates.none { it.exitId == nl.exitId })
        assertEquals(2, candidates.size)
    }

    @Test
    fun `shuffle never offers the exit already in use`() {
        val candidates = shuffleCandidates(relays, currentExitId = de.exitId)
        assertEquals(listOf(fi.exitId), candidates.map { it.exitId })
    }

    @Test
    fun `the only active exit stays offered rather than leaving shuffle inert`() {
        val candidates = shuffleCandidates(listOf(de, nl), currentExitId = de.exitId)
        assertEquals(listOf(de.exitId), candidates.map { it.exitId })
    }

    @Test
    fun `a catalogue with no active exit offers nothing`() {
        assertTrue(shuffleCandidates(listOf(nl), currentExitId = null).isEmpty())
    }

    @Test
    fun `the live tunnel names the current exit, whatever is pinned`() {
        assertEquals(
            de.exitId,
            currentExitId(relays, activeEndpointHost = "185.65.135.10", pin = ExitPin.Automatic),
        )
    }

    @Test
    fun `with no tunnel the pinned exit is the current one`() {
        assertEquals(
            fi.exitId,
            currentExitId(relays, activeEndpointHost = null, pin = ExitPin.Exit(fi.exitId)),
        )
    }

    @Test
    fun `Automatic with no tunnel names no current exit`() {
        assertNull(currentExitId(relays, activeEndpointHost = null, pin = ExitPin.Automatic))
    }

    private fun relay(exitId: String, endpoint: String, country: String, active: Boolean) =
        WarrenRelaySummary(
            exitId = exitId,
            exitPubkeyHex = "ab",
            endpoint = endpoint,
            country = country,
            city = "",
            active = active,
            weight = 1,
        )
}
