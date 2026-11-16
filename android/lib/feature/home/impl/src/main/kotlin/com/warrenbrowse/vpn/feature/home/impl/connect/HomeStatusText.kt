package com.warrenbrowse.vpn.feature.home.impl.connect

import androidx.annotation.DrawableRes
import androidx.annotation.StringRes
import com.warrenbrowse.vpn.lib.model.GeoIpLocation
import com.warrenbrowse.vpn.lib.model.TunnelState
import com.warrenbrowse.vpn.lib.ui.resource.R

/** One line of the home status block, resolved to words by the composable that shows it. */
internal sealed interface HomeLine {
    data class Res(@param:StringRes val id: Int) : HomeLine

    /** A place named city first, the way a ticket does: "Helsinki, Finland". */
    data class Place(val countryCode: String, val city: String?) : HomeLine

    /** The exit named country first on the location row: "Finland, Helsinki". */
    data class ExitPlace(val countryCode: String, val city: String?) : HomeLine

    /** The exit host, set in the mono face; a multihop circuit names its entry too. */
    data class Hostname(val exit: String, val entry: String?) : HomeLine

    data class Literal(val text: String) : HomeLine
}

internal data class HomeStatus(
    @param:StringRes val pill: Int,
    val title: HomeLine,
    val subtitle: HomeLine?,
)

/** The pill, title and subtitle of the home status block (desktop `home-status-text.ts`). */
internal fun homeStatus(
    state: TunnelState,
    hostOffline: Boolean,
    location: GeoIpLocation?,
    includeOnly: Boolean,
): HomeStatus {
    val visible = HomeLine.Res(R.string.you_are_visible)
    val protectedLine = HomeLine.Res(R.string.you_are_protected)
    val place = location?.takeIf { it.country.isNotBlank() }?.let { HomeLine.Place(it.country, it.city) }
    return when (state.connectionPhase(hostOffline)) {
        ConnectionPhase.Exposed ->
            HomeStatus(
                R.string.home_status_unprotected,
                visible,
                HomeLine.Res(R.string.connection_not_encrypted),
            )
        ConnectionPhase.Blocked ->
            HomeStatus(R.string.home_status_blocked, HomeLine.Res(R.string.home_connection_blocked), null)
        ConnectionPhase.Interrupted ->
            HomeStatus(
                R.string.home_status_interrupted,
                HomeLine.Res(R.string.connection_interrupted),
                protectedLine,
            )
        ConnectionPhase.Connecting -> {
            val inProgress = HomeLine.Res(R.string.connection_in_progress)
            when {
                state is TunnelState.Disconnecting ->
                    HomeStatus(
                        R.string.home_status_connecting,
                        visible,
                        HomeLine.Res(R.string.disconnecting),
                    )
                place != null -> HomeStatus(R.string.home_status_connecting, place, inProgress)
                else -> HomeStatus(R.string.home_status_connecting, inProgress, null)
            }
        }
        ConnectionPhase.Protected -> {
            val hostname = location?.hostname
            val subtitle =
                when {
                    includeOnly -> HomeLine.Res(R.string.home_only_selected_apps_protected)
                    hostname != null -> HomeLine.Hostname(hostname, location.entryHostname)
                    else -> protectedLine
                }
            HomeStatus(R.string.home_status_protected, place ?: protectedLine, subtitle)
        }
    }
}

/**
 * A place, city first; a city-state, whose city is its country under any of [countryNames] (the
 * local name, then the English one the relay list spells cities in), is named once.
 */
internal fun placeName(
    city: String?,
    countryNames: List<String>,
    cityCountry: (String, String) -> String,
): String {
    val country = countryNames.first()
    val trimmed = city?.trim()
    return if (trimmed.isNullOrEmpty() || countryNames.any { it.equals(trimmed, ignoreCase = true) }) {
        country
    } else {
        cityCountry(trimmed, country)
    }
}

/**
 * What the location row names (desktop `HomeControls` LocationRow): the exit once a tunnel carries
 * traffic and the exit is known, the selection otherwise; null while the selection is unresolved.
 */
internal fun locationRowLine(
    state: TunnelState,
    location: GeoIpLocation?,
    selectedTitle: String?,
): HomeLine? {
    val tunneled = state is TunnelState.Connected || state is TunnelState.Connecting
    val exit = location?.takeIf { tunneled && it.country.isNotBlank() }
    return when {
        exit != null -> HomeLine.ExitPlace(exit.country, exit.city)
        selectedTitle != null -> HomeLine.Literal(selectedTitle)
        else -> null
    }
}

/** The round picture of the location row. */
internal sealed interface LocationThumb {
    /** A full-width square of [landscape], starting at canvas row [topRow]. */
    data class Art(@param:DrawableRes val landscape: Int, val topRow: Int) : LocationThumb

    data class Flag(val countryCode: String) : LocationThumb
}

/**
 * The location row's picture (desktop `LocationThumb`): the country's own landscape crowned by
 * its skyline, its flag when it has no art, the watched plain when no country is named.
 */
internal fun locationThumb(countryCode: String?): LocationThumb {
    val code = countryCode?.trim()?.lowercase().orEmpty()
    val landscape = SceneryTable.countries[code]
    return when {
        landscape != null -> art(landscape)
        code.isNotEmpty() -> LocationThumb.Flag(code)
        else -> art(SceneryTable.plain)
    }
}

private fun art(@DrawableRes landscape: Int): LocationThumb.Art =
    LocationThumb.Art(
        landscape,
        SceneryFraming.thumbnailTopRow(SceneryTable.landscapes.getValue(landscape).skylineTopRow),
    )
