package com.warrenbrowse.vpn.lib.pushnotification.tunnelstate

import android.content.Context
import com.warrenbrowse.vpn.lib.model.GeoIpLocation
import com.warrenbrowse.vpn.lib.model.NotificationTunnelState
import com.warrenbrowse.vpn.lib.model.PrepareError
import com.warrenbrowse.vpn.lib.ui.resource.R
import io.mockk.every
import io.mockk.mockk
import kotlin.test.assertEquals
import kotlin.test.assertNull
import org.junit.jupiter.api.BeforeEach
import org.junit.jupiter.api.Test

class TunnelStateNotificationCopyTest {

    private lateinit var context: Context

    private fun location(city: String?) =
        GeoIpLocation(
            ipv4 = null,
            ipv6 = null,
            country = "France",
            city = city,
            latitude = 0.0,
            longitude = 0.0,
            hostname = "fr-par-001",
            entryHostname = null,
        )

    @BeforeEach
    fun setup() {
        context = mockk()
        every { context.getString(R.string.notification_disconnected_unsecure) } returns
            "Disconnected and unsecure"
        every { context.getString(R.string.disconnected_vpn_permission_error) } returns
            "Disconnected (No VPN permission)"
        every { context.getString(eq(R.string.notification_connected_to), any()) } answers
            { "Connected to ${secondArg<Array<Any>>()[0]}" }
        every { context.getString(eq(R.string.notification_connecting_to), any()) } answers
            { "Connecting to ${secondArg<Array<Any>>()[0]}" }
        every { context.getString(eq(R.string.country_comma_city), any(), any()) } answers
            { "${secondArg<Array<Any>>()[0]}, ${secondArg<Array<Any>>()[1]}" }
    }

    @Test
    fun `a plain disconnect says the device is unsecure`() {
        assertEquals(
            "Disconnected and unsecure",
            NotificationTunnelState.Disconnected(prepareError = null).notificationTitle(context),
        )
    }

    @Test
    fun `a disconnect caused by a missing permission keeps naming that cause`() {
        assertEquals(
            "Disconnected (No VPN permission)",
            NotificationTunnelState.Disconnected(PrepareError.NotPrepared(mockk()))
                .notificationTitle(context),
        )
    }

    @Test
    fun `the connected title names the location`() {
        assertEquals(
            "Connected to Paris",
            NotificationTunnelState.Connected(location("Paris")).notificationTitle(context),
        )
    }

    @Test
    fun `the connecting title names the location`() {
        assertEquals(
            "Connecting to Paris",
            NotificationTunnelState.Connecting(location("Paris")).notificationTitle(context),
        )
    }

    @Test
    fun `a location without a city falls back to the country`() {
        assertEquals(
            "Connected to France",
            NotificationTunnelState.Connected(location(null)).notificationTitle(context),
        )
    }

    @Test
    fun `an unknown location leaves the bare connected title`() {
        every { context.getString(R.string.connected) } returns "Connected"
        assertEquals(
            "Connected",
            NotificationTunnelState.Connected(location = null).notificationTitle(context),
        )
    }

    @Test
    fun `the detail line carries country and city, never the hostname`() {
        val text = NotificationTunnelState.Connected(location("Paris")).notificationText(context)
        assertEquals("France, Paris", text)
    }

    @Test
    fun `the detail line drops the separator when the city is unknown`() {
        assertEquals(
            "France",
            NotificationTunnelState.Connected(location(null)).notificationText(context),
        )
    }

    @Test
    fun `states without a location have no detail line`() {
        assertNull(NotificationTunnelState.Blocking.notificationText(context))
    }
}
