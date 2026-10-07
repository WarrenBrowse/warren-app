package com.warrenbrowse.vpn.feature.splittunneling.impl.countries

import java.text.Collator
import com.warrenbrowse.vpn.feature.splittunneling.impl.applist.AppData
import com.warrenbrowse.vpn.lib.model.AppExit
import com.warrenbrowse.vpn.lib.model.AppRouteLine
import com.warrenbrowse.vpn.lib.model.AppRouteUnavailableReason
import com.warrenbrowse.vpn.lib.repository.WarrenRelaySummary

/** What a row of the country page, or its button, can do. */
class CountryPickerActions(
    val onSearchChange: (String) -> Unit,
    val onToggleCountry: (String) -> Unit,
    val onChoose: (AppExit) -> Unit,
    val onCancel: () -> Unit,
)

/** The country picker opened for [app]. */
data class CountryPickerUiState(
    val app: AppData,
    val current: AppExit?,
    val searchTerm: String,
    val options: List<CountryOption>,
    /** The countries whose cities are shown. */
    val expanded: Set<String>,
    /** Every exit some app already leaves from. */
    val exitsInUse: Set<AppExit>,
    val catalogueLoaded: Boolean,
)

/** A country with an active server, and its cities with one, by display name. */
data class CountryOption(val country: String, val name: String, val cities: List<String>)

/** The colour of the dot before a route line. */
enum class RouteTone {
    Positive,
    Pending,
    Error,
    Muted,
}

/**
 * The countries and cities a per-app exit can use: those with an active server, by display name.
 * A search matching a country keeps all its cities; one matching only cities keeps those. The
 * twin of the desktop `buildCountryOptions`.
 */
fun buildCountryOptions(
    relays: List<WarrenRelaySummary>,
    searchTerm: String,
    countryName: (String) -> String,
): List<CountryOption> {
    val needle = searchTerm.trim()
    val matches = { name: String -> needle.isEmpty() || name.contains(needle, ignoreCase = true) }
    val collator = Collator.getInstance()
    return relays
        .filter { it.active && it.country.isNotBlank() }
        .groupBy { it.country.trim().lowercase() }
        .map { (country, active) ->
            val cities =
                active
                    .map { it.city.trim() }
                    .filter { it.isNotEmpty() }
                    .distinct()
                    .sortedWith(collator)
            CountryOption(country, countryName(country), cities)
        }
        .mapNotNull { option ->
            when {
                matches(option.name) -> option
                else ->
                    option.cities.filter(matches).takeIf { it.isNotEmpty() }?.let {
                        option.copy(cities = it)
                    }
            }
        }
        .sortedWith(compareBy(collator) { it.name })
}

/** The picker for [app], its options built from [relays]. */
fun countryPicker(
    app: AppData,
    appExits: Map<String, AppExit>,
    relays: List<WarrenRelaySummary>,
    searchTerm: String,
    expanded: Set<String>,
    countryName: (String) -> String,
): CountryPickerUiState {
    val options = buildCountryOptions(relays, searchTerm, countryName)
    return CountryPickerUiState(
        app = app,
        current = appExits[app.packageName.value],
        searchTerm = searchTerm,
        options = options,
        // A search that only matched cities shows them straight away.
        expanded = if (searchTerm.isBlank()) expanded else options.mapTo(HashSet()) { it.country },
        exitsInUse = appExits.values.toSet(),
        catalogueLoaded = relays.isNotEmpty(),
    )
}

/**
 * The chip's text: the country, or "City, Country". The desktop joins the two the same way in
 * every language.
 */
fun exitLabel(exit: AppExit, countryName: (String) -> String): String {
    val country = countryName(exit.country)
    return exit.city?.let { "$it, $country" } ?: country
}

/**
 * A route waiting for the main connection is not a fault, so only a route that cannot run for a
 * reason of its own is red.
 */
fun AppRouteLine.tone(): RouteTone =
    when (this) {
        is AppRouteLine.Connected -> RouteTone.Positive
        AppRouteLine.Connecting,
        AppRouteLine.Waiting,
        AppRouteLine.Blocked -> RouteTone.Pending
        is AppRouteLine.Unavailable ->
            if (reason == AppRouteUnavailableReason.TunnelDown) RouteTone.Pending
            else RouteTone.Error
        AppRouteLine.Paused,
        AppRouteLine.Bypassed -> RouteTone.Muted
    }

/**
 * [text] isolated as left to right, so an address keeps its order inside a right-to-left
 * sentence (Arabic, Persian).
 */
fun isolateLeftToRight(text: String): String = "$LEFT_TO_RIGHT_ISOLATE$text$POP_DIRECTIONAL_ISOLATE"

private const val LEFT_TO_RIGHT_ISOLATE = '\u2066'
private const val POP_DIRECTIONAL_ISOLATE = '\u2069'
