package com.warrenbrowse.vpn.feature.home.impl.connect

import com.warrenbrowse.vpn.lib.model.Endpoint
import com.warrenbrowse.vpn.lib.model.TransportProtocol
import com.warrenbrowse.vpn.lib.repository.ExitPin
import com.warrenbrowse.vpn.lib.repository.WarrenRelaySummary
import java.net.InetSocketAddress
import kotlin.test.assertEquals
import kotlin.test.assertNull
import org.junit.jupiter.api.Test

class ConnectLocationTest {

    private val relays = listOf(
        relay(exitId = "exit-de", endpoint = "185.65.135.10:443", country = "de", city = "Berlin"),
        relay(exitId = "exit-fi", endpoint = "146.70.1.2:443", country = "fi", city = "Helsinki"),
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
        val location = activeExitLocation(endpoint("185.65.135.10", 443), relays)
        assertEquals("de", location?.country)
        assertEquals("Berlin", location?.city)
    }

    @Test
    fun `an endpoint outside the catalogue resolves to nothing`() {
        assertNull(activeExitLocation(endpoint("203.0.113.1", 443), relays))
    }

    private fun endpoint(host: String, port: Int) =
        Endpoint(InetSocketAddress(host, port), TransportProtocol.Udp)

    private fun relay(exitId: String, endpoint: String, country: String, city: String) =
        WarrenRelaySummary(
            exitId = exitId,
            exitPubkeyHex = "ab",
            endpoint = endpoint,
            country = country,
            city = city,
            active = true,
            weight = 1,
        )
}
