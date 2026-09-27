package com.warrenbrowse.vpn.feature.home.impl.connect.connectioninfo

import kotlin.test.assertEquals
import kotlin.test.assertNull
import com.warrenbrowse.vpn.lib.model.AppExit
import com.warrenbrowse.vpn.lib.model.AppRouteState
import com.warrenbrowse.vpn.lib.model.AppRouteStatus
import com.warrenbrowse.vpn.lib.model.AppRouteUnavailableReason
import org.junit.jupiter.api.Test

class AppCountriesSummaryTest {

    private val exits = mapOf("org.chat" to AppExit("se"), "org.maps" to AppExit("de", "Berlin"))

    @Test
    fun `no badge while no app has a country in force`() {
        assertNull(appCountriesSummary(emptyMap(), listOf(route(AppRouteState.Connecting))))
    }

    @Test
    fun `the badge counts the apps with a country in force`() {
        assertEquals(
            AppCountriesSummary(count = 2, anyRouteUnavailable = false),
            appCountriesSummary(exits, listOf(route(AppRouteState.Connected))),
        )
    }

    @Test
    fun `a route waiting for the main connection is not a fault`() {
        val waiting = route(AppRouteState.Unavailable(AppRouteUnavailableReason.TunnelDown))

        assertEquals(false, appCountriesSummary(exits, listOf(waiting))?.anyRouteUnavailable)
    }

    @Test
    fun `a route that cannot run for a reason of its own turns the badge red`() {
        val statuses =
            listOf(
                route(AppRouteState.Connected),
                route(AppRouteState.Unavailable(AppRouteUnavailableReason.WaitingForRoute)),
            )

        assertEquals(true, appCountriesSummary(exits, statuses)?.anyRouteUnavailable)
    }

    private fun route(state: AppRouteState) =
        AppRouteStatus(AppExit("se"), state, publicIp = null, apps = listOf("org.chat"))
}
