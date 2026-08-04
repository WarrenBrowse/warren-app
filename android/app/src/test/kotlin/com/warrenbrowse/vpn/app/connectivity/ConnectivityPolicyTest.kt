package com.warrenbrowse.vpn.app.connectivity

import app.cash.turbine.test
import com.warrenbrowse.talpid.model.Connectivity
import com.warrenbrowse.talpid.model.IpAvailability
import kotlin.time.Duration.Companion.milliseconds
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.runTest
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

@OptIn(ExperimentalCoroutinesApi::class)
class ConnectivityPolicyTest {

    @Test
    fun `offline is never dialable`() {
        assertFalse(Connectivity.Offline.canDialRelay())
    }

    @Test
    fun `ipv6 only online edge is not dialable`() {
        // Relays are dialed over IPv4; a v6-only edge can only produce a
        // doomed Connecting/Error churn (desktop family gating).
        assertFalse(Connectivity.Online(IpAvailability.Ipv6).canDialRelay())
    }

    @Test
    fun `ipv4 bearing connectivity is dialable`() {
        assertTrue(Connectivity.Online(IpAvailability.Ipv4).canDialRelay())
        assertTrue(Connectivity.Online(IpAvailability.Ipv4AndIpv6).canDialRelay())
    }

    @Test
    fun `presume online is dialable`() {
        assertTrue(Connectivity.PresumeOnline.canDialRelay())
    }

    @Test
    fun `rising edge is held and falling edge applies immediately`() = runTest {
        val raw = MutableStateFlow(false)
        raw.holdRisingEdge(1200.milliseconds).test {
            assertEquals(false, awaitItem())
            raw.value = true
            // Inside the hold window nothing is emitted yet.
            expectNoEvents()
            testScheduler.advanceTimeBy(1300)
            assertEquals(true, awaitItem())
            raw.value = false
            assertEquals(false, awaitItem())
        }
    }

    @Test
    fun `a blip shorter than the hold never surfaces`() = runTest {
        val raw = MutableStateFlow(false)
        raw.holdRisingEdge(1200.milliseconds).test {
            assertEquals(false, awaitItem())
            raw.value = true
            testScheduler.advanceTimeBy(500)
            raw.value = false
            testScheduler.advanceTimeBy(5_000)
            // The synthetic handover blip must not flash the offline UI.
            expectNoEvents()
        }
    }
}
