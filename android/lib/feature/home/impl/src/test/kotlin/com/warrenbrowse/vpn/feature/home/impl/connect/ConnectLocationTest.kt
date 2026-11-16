package com.warrenbrowse.vpn.feature.home.impl.connect

import com.warrenbrowse.vpn.lib.model.Endpoint
import com.warrenbrowse.vpn.lib.model.TransportProtocol
import com.warrenbrowse.vpn.lib.model.TunnelEndpoint
import com.warrenbrowse.vpn.lib.repository.ExitPin
import com.warrenbrowse.vpn.lib.repository.WarrenRelaySummary
import java.net.InetSocketAddress
import kotlin.test.assertEquals
import kotlin.test.assertNull
import org.junit.jupiter.api.Test

class ConnectLocationTest {

    private val relays = listOf(
        relay(exitId = "exit-de", endpoint = "185.65.135.10:443", country = "de", city = "Berlin"),
        relay(
            exitId = "exit-fi",
            endpoint = "146.70.1.2:443",
            country = "fi",
            city = "Helsinki",
            pubkeyHex = FI_KEY,
            name = "fi-hel-bveh1",
        ),
    )

    @Test
    fun `Automatic names no location, so nothing is guessed`() {
        assertNull(pinnedExitLocation(ExitPin.Automatic, relays))
    }

    @Test
    fun `a country pin names the country and no city`() {
        val location = pinnedExitLocation(ExitPin.Country("de"), relays)
        assertEquals("de", location?.country)
        assertNull(location?.city)
    }

    @Test
    fun `a city pin names the city it pinned`() {
        val location = pinnedExitLocation(ExitPin.City("fi", "Helsinki"), relays)
        assertEquals("fi", location?.country)
        assertEquals("Helsinki", location?.city)
    }

    @Test
    fun `an exit pin resolves through the catalogue`() {
        val location = pinnedExitLocation(ExitPin.Exit("exit-fi"), relays)
        assertEquals("fi", location?.country)
        assertEquals("Helsinki", location?.city)
    }

    @Test
    fun `an exit pin the catalogue does not know names nothing`() {
        assertNull(pinnedExitLocation(ExitPin.Exit("exit-gone"), relays))
    }

    @Test
    fun `the dialled endpoint resolves to the exit actually being used`() {
        val location = activeExitLocation(tunnel(endpoint("185.65.135.10", 443)), relays)
        assertEquals("de", location?.country)
        assertEquals("Berlin", location?.city)
    }

    @Test
    fun `an endpoint outside the catalogue resolves to nothing`() {
        assertNull(activeExitLocation(tunnel(endpoint("203.0.113.1", 443)), relays))
    }

    @Test
    fun `the live exit is named by its fleet name`() {
        val location = activeExitLocation(tunnel(endpoint("146.70.1.2", 443), FI_KEY), relays)
        assertEquals("fi-hel-bveh1", location?.hostname)
    }

    @Test
    fun `an unnamed live exit is named by its key`() {
        val location = activeExitLocation(tunnel(endpoint("185.65.135.10", 443)), relays)
        assertEquals("warren-" + DE_KEY.take(16), location?.hostname)
    }

    @Test
    fun `the exit key tells apart two exits behind one address`() {
        val sibling =
            relay(
                exitId = "exit-fi-2",
                endpoint = "146.70.1.2:8443",
                country = "fi",
                city = "Helsinki",
                pubkeyHex = "77".repeat(32),
                name = "fi-hel-bveh2",
            )
        val location =
            activeExitLocation(tunnel(endpoint("146.70.1.2", 443), "77".repeat(32)), relays + sibling)
        assertEquals("fi-hel-bveh2", location?.hostname)
    }

    @Test
    fun `a multihop circuit is named by its exit, never by its entry`() {
        val circuit =
            TunnelEndpoint(
                entryEndpoint = endpoint("185.65.135.10", 443),
                endpoint = endpoint("146.70.1.2", 443),
                quantumResistant = false,
                obfuscation = null,
                daita = false,
                exitPubkeyHex = FI_KEY,
            )
        val location = activeExitLocation(circuit, relays)
        assertEquals("fi-hel-bveh1", location?.hostname)
        assertEquals("fi", location?.country)
        assertNull(location?.entryHostname)
    }

    private fun tunnel(exit: Endpoint, exitPubkeyHex: String? = null) =
        TunnelEndpoint(
            entryEndpoint = null,
            endpoint = exit,
            quantumResistant = false,
            obfuscation = null,
            daita = false,
            exitPubkeyHex = exitPubkeyHex,
        )

    private fun endpoint(host: String, port: Int) =
        Endpoint(InetSocketAddress(host, port), TransportProtocol.Udp)

    private fun relay(
        exitId: String,
        endpoint: String,
        country: String,
        city: String,
        pubkeyHex: String = DE_KEY,
        name: String? = null,
    ) =
        WarrenRelaySummary(
            exitId = exitId,
            exitPubkeyHex = pubkeyHex,
            endpoint = endpoint,
            country = country,
            city = city,
            active = true,
            weight = 1,
            name = name,
        )

    private companion object {
        val DE_KEY = "de".repeat(32)
        val FI_KEY = "f1".repeat(32)
    }
}
