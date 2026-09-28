package com.warrenbrowse.vpn.feature.splittunneling.impl

import com.warrenbrowse.vpn.feature.splittunneling.impl.applist.AppData
import com.warrenbrowse.vpn.feature.splittunneling.impl.countries.CountryPickerUiState
import com.warrenbrowse.vpn.lib.model.AppRoute
import com.warrenbrowse.vpn.lib.model.AppRouteLine
import com.warrenbrowse.vpn.lib.model.DefaultRoute
import com.warrenbrowse.vpn.lib.model.asAppRoute

data class Loading(val isModal: Boolean = false)

/**
 * App routing (docs/app-routing.md section 3.5): what the apps without a rule do, and one rule per
 * app whose route differs from that. The route, add and country pages open over the list.
 */
data class AppRoutingUiState(
    val defaultRoute: DefaultRoute = DefaultRoute.Vpn,
    /** The apps on the device whose route differs from [defaultRoute], by name. */
    val rules: List<AppRuleItem> = emptyList(),
    /**
     * Direct is the default and no app of the device is in the VPN list, so the tunnel carries
     * every app: Android captures every app when it is given no allowed one (section 3.4).
     */
    val fullTunnelFallback: Boolean = false,
    /** False below Android 10, where the owner of a flow cannot be looked up. */
    val countrySupported: Boolean = true,
    val isModal: Boolean = false,
    val page: AppRoutingPage = AppRoutingPage.Rules,
    /** A change waiting for the user's answer because it narrows the tunnel to a list. */
    val confirmation: NarrowingConfirmation? = null,
) {
    val showNoRules: Boolean
        get() = rules.isEmpty() && defaultRoute == DefaultRoute.Vpn

    /**
     * Whether some app is outside the VPN, so the system's "Block connections without VPN" leaves
     * it without Internet.
     */
    val someAppOutside: Boolean
        get() = defaultRoute == DefaultRoute.Direct || rules.any { it.route == AppRoute.Direct }
}

/** One row of the list: an app, its route, and for a country the state of its route. */
data class AppRuleItem(val app: AppData, val route: AppRoute, val line: AppRouteLine?)

sealed interface AppRoutingPage {
    /** The default route and the rules. */
    data object Rules : AppRoutingPage

    /** The apps without a rule, to open the route of one. */
    data class AddApp(
        val searchTerm: String,
        val showSystemApps: Boolean,
        val apps: List<AppData>,
    ) : AppRoutingPage

    /** The route of [app]: through the VPN, through a country, or outside the VPN. */
    data class Route(
        val app: AppData,
        val route: AppRoute,
        val defaultRoute: DefaultRoute,
        val line: AppRouteLine?,
    ) : AppRoutingPage {
        val hasRule: Boolean
            get() = route != defaultRoute.asAppRoute()
    }

    /** The countries and cities one app can leave from. */
    data class Country(val picker: CountryPickerUiState) : AppRoutingPage
}

/**
 * Asked before a change makes a few apps the only ones in the VPN, where every app uses it now:
 * [app] for a rule of one app, null for the "Outside the VPN" default over apps with a country.
 */
data class NarrowingConfirmation(val app: AppData?)
