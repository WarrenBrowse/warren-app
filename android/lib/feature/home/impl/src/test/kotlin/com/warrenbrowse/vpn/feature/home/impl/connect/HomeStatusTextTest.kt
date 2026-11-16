package com.warrenbrowse.vpn.feature.home.impl.connect

import com.warrenbrowse.vpn.lib.model.ActionAfterDisconnect
import com.warrenbrowse.vpn.lib.model.ErrorState
import com.warrenbrowse.vpn.lib.model.ErrorStateCause
import com.warrenbrowse.vpn.lib.model.GeoIpLocation
import com.warrenbrowse.vpn.lib.model.TunnelState
import com.warrenbrowse.vpn.lib.ui.resource.R
import io.mockk.mockk
import kotlin.test.assertEquals
import kotlin.test.assertNull
import org.junit.jupiter.api.Test

/**
 * The home status block, word for word the desktop `home-status-text.ts`: the pill names the
 * phase, the title says where the traffic goes or that it goes nowhere safe, the subtitle gives the
 * reason, or the exit host in the mono face once protected.
 */
class HomeStatusTextTest {

    private val helsinki = location(country = "fi", city = "Helsinki", hostname = "warren-3cb7")
    private val connected = TunnelState.Connected(mockk(relaxed = true), null, emptyList())
    private val connecting = TunnelState.Connecting(null, null, emptyList())

    @Test
    fun `exposed says the user is visible and why`() {
        assertEquals(
            HomeStatus(
                pill = R.string.home_status_unprotected,
                title = HomeLine.Res(R.string.you_are_visible),
                subtitle = HomeLine.Res(R.string.connection_not_encrypted),
            ),
            homeStatus(TunnelState.Disconnected(), false, helsinki, includeOnly = false),
        )
    }

    @Test
    fun `connecting names the target place and that it is in progress`() {
        assertEquals(
            HomeStatus(
                pill = R.string.home_status_connecting,
                title = HomeLine.Place("fi", "Helsinki"),
                subtitle = HomeLine.Res(R.string.connection_in_progress),
            ),
            homeStatus(connecting, false, helsinki, includeOnly = false),
        )
    }

    @Test
    fun `connecting with no target yet titles the progress itself`() {
        assertEquals(
            HomeStatus(
                pill = R.string.home_status_connecting,
                title = HomeLine.Res(R.string.connection_in_progress),
                subtitle = null,
            ),
            homeStatus(connecting, false, null, includeOnly = false),
        )
    }

    @Test
    fun `every teardown reads as visible while disconnecting, as on desktop`() {
        val expected =
            HomeStatus(
                pill = R.string.home_status_connecting,
                title = HomeLine.Res(R.string.you_are_visible),
                subtitle = HomeLine.Res(R.string.disconnecting),
            )
        ActionAfterDisconnect.entries.forEach { after ->
            assertEquals(
                expected,
                homeStatus(TunnelState.Disconnecting(after), false, helsinki, includeOnly = false),
                "$after",
            )
        }
    }

    @Test
    fun `protected titles the place and puts the exit host under it`() {
        assertEquals(
            HomeStatus(
                pill = R.string.home_status_protected,
                title = HomeLine.Place("fi", "Helsinki"),
                subtitle = HomeLine.Hostname("warren-3cb7", entry = null),
            ),
            homeStatus(connected, false, helsinki, includeOnly = false),
        )
    }

    @Test
    fun `a multihop host names the exit via the entry`() {
        val multihop = helsinki.copy(entryHostname = "warren-entry")
        assertEquals(
            HomeLine.Hostname("warren-3cb7", entry = "warren-entry"),
            homeStatus(connected, false, multihop, includeOnly = false).subtitle,
        )
    }

    @Test
    fun `protected without a host or a place still says the user is protected`() {
        assertEquals(
            HomeStatus(
                pill = R.string.home_status_protected,
                title = HomeLine.Res(R.string.you_are_protected),
                subtitle = HomeLine.Res(R.string.you_are_protected),
            ),
            homeStatus(connected, false, null, includeOnly = false),
        )
    }

    @Test
    fun `include-only says only the chosen apps are protected instead of the host`() {
        assertEquals(
            HomeLine.Res(R.string.home_only_selected_apps_protected),
            homeStatus(connected, false, helsinki, includeOnly = true).subtitle,
        )
    }

    @Test
    fun `an offline host interrupts a connected tunnel`() {
        assertEquals(
            HomeStatus(
                pill = R.string.home_status_interrupted,
                title = HomeLine.Res(R.string.connection_interrupted),
                subtitle = HomeLine.Res(R.string.you_are_protected),
            ),
            homeStatus(connected, true, helsinki, includeOnly = false),
        )
    }

    @Test
    fun `a blocking error is a blocked connection with no subtitle`() {
        val blocking = TunnelState.Error(ErrorState(ErrorStateCause.StartTunnelError, true))
        assertEquals(
            HomeStatus(
                pill = R.string.home_status_blocked,
                title = HomeLine.Res(R.string.home_connection_blocked),
                subtitle = null,
            ),
            homeStatus(blocking, false, helsinki, includeOnly = false),
        )
    }

    @Test
    fun `a place is named city first, and a city-state once`() {
        val comma = { a: String, b: String -> "$a, $b" }
        assertEquals("Helsinki, Finland", placeName("Helsinki", listOf("Finland"), comma))
        assertEquals("Finland", placeName(null, listOf("Finland"), comma))
        assertEquals("Finland", placeName(" ", listOf("Finland"), comma))
        // The relay list spells the city in English, the country name follows the locale.
        assertEquals("Singapour", placeName("Singapore", listOf("Singapour", "Singapore"), comma))
    }

    @Test
    fun `the location row names the exit country first once a tunnel carries traffic`() {
        assertEquals(
            HomeLine.ExitPlace("fi", "Helsinki"),
            locationRowLine(connected, helsinki, selectedTitle = "Automatic"),
        )
        assertEquals(
            HomeLine.ExitPlace("fi", "Helsinki"),
            locationRowLine(connecting, helsinki, selectedTitle = "Automatic"),
        )
    }

    @Test
    fun `the location row names the selection while no tunnel carries traffic`() {
        assertEquals(
            HomeLine.Literal("Automatic"),
            locationRowLine(TunnelState.Disconnected(), helsinki, selectedTitle = "Automatic"),
        )
        assertEquals(
            HomeLine.Literal("Finland"),
            locationRowLine(connecting, null, selectedTitle = "Finland"),
        )
        assertNull(locationRowLine(TunnelState.Disconnected(), null, selectedTitle = null))
    }

    private fun location(country: String, city: String?, hostname: String?) =
        GeoIpLocation(
            ipv4 = null,
            ipv6 = null,
            country = country,
            city = city,
            latitude = 0.0,
            longitude = 0.0,
            hostname = hostname,
            entryHostname = null,
        )
}
