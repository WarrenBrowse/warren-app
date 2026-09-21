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
    fun `ipv6 only online edge is not dialable against a v4 only fleet`() {
        // The fleet every deployment had until 2026: an IPv6-only edge can only
        // produce a doomed Connecting/Error churn (desktop family gating).
        assertFalse(Connectivity.Online(IpAvailability.Ipv6).canDialRelay())
    }

    @Test
    fun `ipv6 only online edge is dialable once the fleet publishes ipv6`() {
        // The whole point of measuring instead of assuming: the same device on
        // the same network stops being walled the moment an entry hop binds a
        // v6 listener, with no further client change.
        val dualStackFleet = RelayFamilies(RelayFamilies.IPV4 or RelayFamilies.IPV6)
        assertTrue(Connectivity.Online(IpAvailability.Ipv6).canDialRelay(dualStackFleet))
        assertFalse(
            Connectivity.Online(IpAvailability.Ipv6).isOnlineWithNoDialableFamily(dualStackFleet)
        )
    }

    @Test
    fun `a v4 only device is walled by a v6 only fleet`() {
        // The mirror case, which the old hardcoded assumption could not even
        // express: the comparison runs both ways.
        val v6Fleet = RelayFamilies(RelayFamilies.IPV6)
        assertFalse(Connectivity.Online(IpAvailability.Ipv4).canDialRelay(v6Fleet))
        assertTrue(Connectivity.Online(IpAvailability.Ipv4).isOnlineWithNoDialableFamily(v6Fleet))
    }

    @Test
    fun `a dual stack device dials whatever the fleet publishes`() {
        for (mask in listOf(RelayFamilies.IPV4, RelayFamilies.IPV6)) {
            assertTrue(
                Connectivity.Online(IpAvailability.Ipv4AndIpv6).canDialRelay(RelayFamilies(mask))
            )
        }
    }

    @Test
    fun `a fleet that publishes nothing never parks a device`() {
        // An empty or unverifiable directory is a fleet-side fault. Parking on
        // it would wait for a network event that cannot fix anything.
        val nothing = RelayFamilies(0)
        assertTrue(Connectivity.Online(IpAvailability.Ipv6).canDialRelay(nothing))
        assertFalse(Connectivity.Online(IpAvailability.Ipv6).isOnlineWithNoDialableFamily(nothing))
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
    fun `an ipv6 only edge is online with no dialable family`() {
        // The distinction the user sees: their phone has working internet and
        // Warren still cannot connect, so "you are offline" would be a lie.
        assertTrue(Connectivity.Online(IpAvailability.Ipv6).isOnlineWithNoDialableFamily())
    }

    @Test
    fun `offline and dialable edges are not online with no dialable family`() {
        assertFalse(Connectivity.Offline.isOnlineWithNoDialableFamily())
        assertFalse(Connectivity.Online(IpAvailability.Ipv4).isOnlineWithNoDialableFamily())
        assertFalse(
            Connectivity.Online(IpAvailability.Ipv4AndIpv6).isOnlineWithNoDialableFamily()
        )
        // PresumeOnline means the platform could not resolve the state: it is
        // dialed rather than named as a dead network.
        assertFalse(Connectivity.PresumeOnline.isOnlineWithNoDialableFamily())
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
