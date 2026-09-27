package com.warrenbrowse.vpn.feature.home.impl.connect.connectioninfo

import com.warrenbrowse.vpn.lib.model.AppExit
import com.warrenbrowse.vpn.lib.model.AppRouteState
import com.warrenbrowse.vpn.lib.model.AppRouteStatus
import com.warrenbrowse.vpn.lib.model.AppRouteUnavailableReason

/**
 * The "N apps in other countries" badge (desktop `AppCountriesIndicator`): how many apps leave
 * from a country of their own, and whether one of their routes cannot run.
 */
data class AppCountriesSummary(val count: Int, val anyRouteUnavailable: Boolean)

/**
 * The badge for the countries in force, or null when no app has one. A route waiting for the main
 * connection is not a fault; one waiting for a free route is, since its apps do not leave from
 * their country meanwhile.
 */
fun appCountriesSummary(
    effectiveAppExits: Map<String, AppExit>,
    statuses: List<AppRouteStatus>,
): AppCountriesSummary? =
    effectiveAppExits.size
        .takeIf { it > 0 }
        ?.let { count ->
            AppCountriesSummary(
                count = count,
                anyRouteUnavailable =
                    statuses.any {
                        val state = it.state
                        state is AppRouteState.Unavailable &&
                            state.reason != AppRouteUnavailableReason.TunnelDown
                    },
            )
        }
