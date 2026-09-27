package com.warrenbrowse.vpn.feature.settings.impl

import com.warrenbrowse.vpn.lib.model.countryDisplayName
import com.warrenbrowse.vpn.lib.repository.ExitPin
import com.warrenbrowse.vpn.lib.repository.WarrenConnectedInfo
import com.warrenbrowse.vpn.lib.repository.WarrenRelaySummary
import com.warrenbrowse.vpn.lib.repository.pinsCity
import com.warrenbrowse.vpn.lib.repository.pinsCountry
import com.warrenbrowse.vpn.lib.ui.designsystem.Position

/** Which hop the picker is choosing while multi-hop is on. */
internal enum class PickerScope {
    Entry,
    Exit,
}

/** What a pick must do to the tunnel once the new selection is persisted. */
internal enum class PickFollowUp {
    /** A tunnel is up: re-dial it so the new selection takes effect now. */
    Reconnect,

    /** No tunnel: selecting a location IS the connect gesture, as on desktop. */
    Connect,

    /** A transition is already in flight; queueing a second one would thrash. */
    None,
}

/** Stable key identifying a city inside the accordion (city names repeat across countries). */
internal fun cityKey(country: String, city: String): String = "$country $city"

/**
 * What a pick does to the tunnel. Desktop connects on every pick whatever the
 * state; the only case Android must hold back on is a transition that is
 * already running, because a second dial would tear down the one in progress.
 */
internal fun pickFollowUp(tunnel: WarrenConnectedInfo): PickFollowUp = when (tunnel) {
    is WarrenConnectedInfo.Connected -> PickFollowUp.Reconnect
    is WarrenConnectedInfo.Dialling,
    is WarrenConnectedInfo.Disconnecting -> PickFollowUp.None
    is WarrenConnectedInfo.Disconnected,
    is WarrenConnectedInfo.Failed,
    is WarrenConnectedInfo.Blocking -> PickFollowUp.Connect
}

/**
 * True while the tunnel is mid-transition. The picker greys its rows out then:
 * the pick pops the screen, but the user can re-open it during the transition
 * and a second tap would queue another dial behind the first.
 */
internal fun transitionInFlight(tunnel: WarrenConnectedInfo): Boolean =
    tunnel is WarrenConnectedInfo.Dialling || tunnel is WarrenConnectedInfo.Disconnecting

/**
 * Apply an entry-hop pick ([country] `null` = automatic) and return the hop the
 * picker must show next. Desktop auto-advances to the exit hop so the pair is
 * chosen in one visit instead of two trips into the picker.
 */
internal fun applyEntryPick(country: String?, setEntryCountry: (String?) -> Unit): PickerScope {
    setEntryCountry(country)
    return PickerScope.Exit
}

/** The accordion branches that are open: countries, cities and custom lists. */
internal data class ExpandedKeys(
    val countries: Set<String>,
    val cities: Set<String>,
    val lists: Set<String> = emptySet(),
)

/**
 * Which branches to expand when the picker opens on [pin]. A pin deeper than a
 * country needs its ancestors open too, otherwise the scroll-to-selected has no
 * row to land on.
 */
internal fun expandedKeysFor(pin: ExitPin, relays: List<WarrenRelaySummary>): ExpandedKeys =
    when (pin) {
        ExitPin.Automatic -> ExpandedKeys(emptySet(), emptySet())
        is ExitPin.Country -> ExpandedKeys(setOf(pin.country), emptySet())
        is ExitPin.City ->
            ExpandedKeys(setOf(pin.country), setOf(cityKey(pin.country, pin.city)))
        is ExitPin.Exit ->
            relays.firstOrNull { it.exitId == pin.exitId }?.let {
                ExpandedKeys(setOf(it.country), setOf(cityKey(it.country, it.city)))
            } ?: ExpandedKeys(emptySet(), emptySet())
    }

/**
 * Characters typed before the list starts filtering. One character matches
 * almost everything, and because a search force-expands every branch, it is
 * also the query that builds the largest row list the screen can produce.
 */
private const val MIN_SEARCH_LENGTH = 2

/** The query actually applied to the list, empty while below the threshold. */
internal fun appliedQuery(raw: String): String =
    raw.trim().let { if (it.length >= MIN_SEARCH_LENGTH) it else "" }

/**
 * Search predicate. Deliberately geographical: the catalogue carries no
 * hostname, only an endpoint address, and an address is never shown to the
 * user, so matching it would answer a query with a row the user cannot read.
 */
internal fun relayMatches(relay: WarrenRelaySummary, query: String): Boolean =
    query.isEmpty() ||
        relay.country.contains(query, ignoreCase = true) ||
        countryDisplayName(relay.country).contains(query, ignoreCase = true) ||
        relay.city.contains(query, ignoreCase = true)

/** A custom list resolved against the catalogue, ready to render. */
internal data class CustomListSection(val name: String, val places: List<Place>)

/** The catalogue exits a location stands for. */
internal fun relaysIn(location: ExitPin, relays: List<WarrenRelaySummary>): List<WarrenRelaySummary> =
    when (location) {
        ExitPin.Automatic -> emptyList()
        is ExitPin.Country -> relays.filter { it.country.equals(location.country, ignoreCase = true) }
        is ExitPin.City ->
            relays.filter {
                it.country.equals(location.country, ignoreCase = true) &&
                    it.city.equals(location.city, ignoreCase = true)
            }
        is ExitPin.Exit -> relays.filter { it.exitId == location.exitId }
    }

/**
 * Custom lists worth a row. A list exists to hold locations, so one holding
 * none the catalogue still serves is left out (it stays reachable from every
 * row's lists menu, to be filled again). Under a [query], a list whose name
 * matches keeps all its members; otherwise only the matching members are kept
 * and a list left with nothing drops out, so the user's own grouping stays
 * usable exactly when they are searching for something inside it.
 */
internal fun visibleCustomLists(
    customLists: Map<String, List<ExitPin>>,
    relays: List<WarrenRelaySummary>,
    query: String,
): List<CustomListSection> = customLists.mapNotNull { (name, members) ->
    val places = members.mapNotNull { resolvePlace(it, relays) }
    val kept = when {
        query.isEmpty() -> places
        name.contains(query, ignoreCase = true) -> places
        else -> places.filter { place -> relaysIn(place.location, relays).any { relayMatches(it, query) } }
    }
    kept.takeIf { it.isNotEmpty() }?.let { CustomListSection(name, it) }
}

/** Which section a saved location row belongs to, keeping its key unique across sections. */
internal sealed interface ExitSection {
    val keyPrefix: String

    data object Recents : ExitSection {
        override val keyPrefix = "recent"
    }

    data class Custom(val name: String) : ExitSection {
        override val keyPrefix = "custom-$name"
    }
}

/**
 * A location resolved against the catalogue into what its row shows. Every
 * row standing for a place outside the tree (recents, list members) is drawn
 * from one of these, so they all share the tree's anatomy: the country flag,
 * the name, the parent country (or, for a country served by one exit, its
 * city) as a muted subtitle, and the load badge when the place is one exit.
 */
internal data class Place(
    val location: ExitPin,
    val title: String,
    /** Rank of the exit among the exits sharing its city, `null` when alone or not an exit. */
    val ordinal: Int?,
    val subtitle: String?,
    val flagCountry: String?,
    /** The exit whose load the row shows, set only when the place holds exactly one exit. */
    val loadExitId: String?,
    val hasActive: Boolean,
)

/** Stable key of a location, shared by row keys in every section. */
internal fun locationKey(location: ExitPin): String =
    when (location) {
        ExitPin.Automatic -> "automatic"
        is ExitPin.Country -> "country-" + location.country.lowercase()
        is ExitPin.City -> "city-" + cityKey(location.country.lowercase(), location.city.lowercase())
        is ExitPin.Exit -> "exit-" + location.exitId
    }

/**
 * Flat rows the picker renders, so the pinned row has a stable scroll index.
 *
 * A row that rounds into a block with its neighbours carries a [position];
 * [assignPositions] fills it in from the runs the builder produced, so section
 * headers and gaps are what break a block rather than a per-row guess.
 */
internal sealed interface PickerRow {
    val key: String

    /** True when this row carries the current selection (drives the check + scroll). */
    val isPinned: Boolean
        get() = false

    /** Non-null on rows that round into a block; `null` on separators and headers. */
    val position: Position?
        get() = null

    /** True on the rows that open onto children, the only ones drawn with an expand chevron. */
    val expandable: Boolean
        get() = false

    fun withPosition(position: Position): PickerRow = this

    data class Gap(override val key: String) : PickerRow

    data object RecentsHeader : PickerRow {
        override val key = "hdr-recents"
    }

    data object CustomListsHeader : PickerRow {
        override val key = "hdr-custom-lists"
    }

    data object AllLocationsHeader : PickerRow {
        override val key = "hdr-all-locations"
    }

    /**
     * A custom list: a list glyph in the flag slot, its name over its location
     * count, and a chevron folding its members. It never holds the selection.
     */
    data class CustomListRow(
        val name: String,
        val count: Int,
        val expanded: Boolean,
        override val position: Position = Position.Single,
    ) : PickerRow {
        override val key = "list-$name"

        override val expandable = true

        override fun withPosition(position: Position) = copy(position = position)
    }

    data class ExitAutomaticRow(
        override val isPinned: Boolean,
        override val position: Position = Position.Single,
    ) : PickerRow {
        override val key = "automatic-exit"

        override fun withPosition(position: Position) = copy(position = position)
    }

    data class EntryAutomaticRow(
        override val isPinned: Boolean,
        override val position: Position = Position.Single,
    ) : PickerRow {
        override val key = "automatic-entry"

        override fun withPosition(position: Position) = copy(position = position)
    }

    data class EntryCountryRow(
        val country: String,
        val display: String,
        override val isPinned: Boolean,
        override val position: Position = Position.Single,
    ) : PickerRow {
        override val key = "entry-$country"

        override fun withPosition(position: Position) = copy(position = position)
    }

    data class CountryHeader(
        val country: String,
        val display: String,
        val expanded: Boolean,
        val hasActive: Boolean,
        override val isPinned: Boolean,
        override val position: Position = Position.Single,
    ) : PickerRow {
        override val key = "country-$country"

        override val expandable = true

        /** What this row's lists menu adds or removes. */
        val location: ExitPin
            get() = ExitPin.Country(country)

        override fun withPosition(position: Position) = copy(position = position)
    }

    data class CityHeader(
        val country: String,
        val city: String,
        val expanded: Boolean,
        val hasActive: Boolean,
        override val isPinned: Boolean,
        override val position: Position = Position.Single,
    ) : PickerRow {
        override val key = "cityhdr-$country-$city"

        override val expandable = true

        /** Nesting depth, rendered as a design-system hierarchy rather than a card inset. */
        val depth: Int = 1

        /** What this row's lists menu adds or removes. */
        val location: ExitPin
            get() = ExitPin.City(country, city)

        override fun withPosition(position: Position) = copy(position = position)
    }

    /**
     * A location of the country tree standing for exactly one exit: a lone
     * exit in its city, one of several numbered by [ordinal], or a whole
     * country served by a single exit (then at depth 0, with its flag and its
     * city as [subtitle]).
     */
    data class ExitRow(
        val relay: WarrenRelaySummary,
        val title: String,
        /** Rank of this exit among the exits sharing its city, `null` when alone. */
        val ordinal: Int?,
        val depth: Int,
        override val isPinned: Boolean,
        override val position: Position = Position.Single,
        /** The city under the country name, on a row standing for a country with one exit. */
        val subtitle: String? = null,
    ) : PickerRow {
        override val key = "exit-${relay.exitId}"

        /** A row at the top of the tree leads with its country's flag; a nested one does not. */
        val flagCountry: String?
            get() = relay.country.takeIf { depth == 0 }

        /** What this row's lists menu adds or removes. */
        val location: ExitPin
            get() = ExitPin.Exit(relay.exitId)

        override fun withPosition(position: Position) = copy(position = position)
    }

    /**
     * A location saved outside the tree: a recent (depth 0) or a custom list
     * member (depth 1, under its list). Tapping it applies [Place.location].
     */
    data class SavedRow(
        val place: Place,
        val section: ExitSection,
        val depth: Int,
        override val isPinned: Boolean,
        override val position: Position = Position.Single,
    ) : PickerRow {
        override val key = "${section.keyPrefix}-${locationKey(place.location)}"

        override fun withPosition(position: Position) = copy(position = position)
    }
}

/** Position of an item within a block so its corners round into that block. */
internal fun positionOf(index: Int, count: Int): Position = when {
    count <= 1 -> Position.Single
    index == 0 -> Position.Top
    index == count - 1 -> Position.Bottom
    else -> Position.Middle
}

/**
 * Round every run of consecutive positionable rows into one block. Countries
 * that are merely collapsed then read as a single list the way desktop lays
 * them out, instead of one isolated pill each.
 */
internal fun assignPositions(rows: List<PickerRow>): List<PickerRow> {
    val out = ArrayList<PickerRow>(rows.size)
    var i = 0
    while (i < rows.size) {
        if (rows[i].position == null) {
            out.add(rows[i])
            i++
            continue
        }
        var end = i
        while (end < rows.size && rows[end].position != null) end++
        val count = end - i
        for (k in i until end) out.add(rows[k].withPosition(positionOf(k - i, count)))
        i = end
    }
    return out
}

/**
 * Everything the row list is computed from, held by value so the composable
 * recomputes the rows only when one of them changes, never on a recomposition
 * caused by something else (a tunnel edge, a snackbar).
 */
internal data class PickerInputs(
    val relays: List<WarrenRelaySummary>,
    /** The applied query (see [appliedQuery]), empty when not searching. */
    val query: String,
    val scope: PickerScope,
    val entryCountry: String?,
    val recentsEnabled: Boolean,
    val recentPins: List<ExitPin>,
    val customLists: Map<String, List<ExitPin>>,
    val exitPin: ExitPin,
    val expanded: ExpandedKeys,
)

/** The picker's rows for [inputs], in render order. Pure: no composition state. */
internal fun pickerRows(inputs: PickerInputs): List<PickerRow> =
    with(inputs) {
        val searching = query.isNotEmpty()
        if (scope == PickerScope.Entry) {
            assignPositions(
                buildEntryRows(
                    countries = entryCountriesOf(relays, query),
                    entryCountry = entryCountry,
                    notSearching = !searching,
                )
            )
        } else {
            val filtered = relays.filter { relayMatches(it, query) }
            val recents =
                if (!searching && recentsEnabled) recentPlaces(recentPins, relays) else emptyList()
            // country -> (city -> relays), both ordered by localized name.
            val byCountry: Map<String, Map<String, List<WarrenRelaySummary>>> =
                filtered
                    .sortedWith(compareBy({ countryDisplayName(it.country) }, { it.city }))
                    .groupBy { it.country }
                    .mapValues { (_, rs) -> rs.groupBy { it.city } }
            assignPositions(
                buildPickerRows(
                    query = query,
                    recents = recents,
                    customLists = visibleCustomLists(customLists, relays, query),
                    byCountry = byCountry,
                    exitPin = exitPin,
                    expanded = expanded,
                )
            )
        }
    }

/** Distinct catalogue countries for the entry hop, ordered by localized name. */
internal fun entryCountriesOf(relays: List<WarrenRelaySummary>, query: String): List<String> =
    relays
        .map { it.country }
        .filter { it.isNotBlank() }
        .distinct()
        .filter {
            query.isEmpty() ||
                it.contains(query, ignoreCase = true) ||
                countryDisplayName(it).contains(query, ignoreCase = true)
        }
        .sortedBy { countryDisplayName(it) }

/** Entry-hop list: an explicit Automatic row then one row per catalogue country. */
internal fun buildEntryRows(
    countries: List<String>,
    entryCountry: String?,
    notSearching: Boolean,
): List<PickerRow> = buildList {
    if (notSearching) {
        add(PickerRow.EntryAutomaticRow(isPinned = entryCountry.isNullOrBlank()))
        add(PickerRow.Gap("gap-entry-automatic"))
    }
    countries.forEach { country ->
        add(
            PickerRow.EntryCountryRow(
                country = country,
                display = countryDisplayName(country),
                isPinned = country.equals(entryCountry, ignoreCase = true),
            )
        )
    }
}

internal fun exitTitle(relay: WarrenRelaySummary): String =
    relay.city.ifBlank { countryDisplayName(relay.country) }

/**
 * [location] as its row shows it, or `null` when the catalogue no longer
 * serves it. A place whose exits are all down stays (inactive), as its tree
 * row does.
 */
internal fun resolvePlace(location: ExitPin, relays: List<WarrenRelaySummary>): Place? {
    val inScope = relaysIn(location, relays)
    val first = inScope.firstOrNull() ?: return null
    val country = first.country
    val countryName = countryDisplayName(country)
    val loadExitId = inScope.singleOrNull()?.exitId
    val hasActive = inScope.any { it.active }
    return when (location) {
        ExitPin.Automatic -> null
        is ExitPin.Country ->
            Place(
                location = location,
                title = countryName,
                ordinal = null,
                // A country served by one exit names its city, as its tree row does.
                subtitle = if (loadExitId != null) first.city.ifBlank { null } else null,
                flagCountry = country,
                loadExitId = loadExitId,
                hasActive = hasActive,
            )
        is ExitPin.City ->
            Place(
                location = location,
                title = first.city.ifBlank { countryName },
                ordinal = null,
                subtitle = countryName.takeIf { first.city.isNotBlank() },
                flagCountry = country,
                loadExitId = loadExitId,
                hasActive = hasActive,
            )
        is ExitPin.Exit -> exitPlace(location, first, relays)
    }
}

/** An exit named as the tree names it: its country when alone there, else its city and rank. */
private fun exitPlace(location: ExitPin.Exit, relay: WarrenRelaySummary, relays: List<WarrenRelaySummary>): Place {
    val countryName = countryDisplayName(relay.country)
    val aloneInCountry = relays.count { it.country.equals(relay.country, ignoreCase = true) } == 1
    val cityMates =
        relaysIn(ExitPin.City(relay.country, relay.city), relays).sortedBy { it.exitId }
    return Place(
        location = location,
        title = if (aloneInCountry) countryName else exitTitle(relay),
        ordinal = if (!aloneInCountry && cityMates.size > 1) cityMates.indexOf(relay) + 1 else null,
        subtitle = when {
            aloneInCountry -> relay.city.ifBlank { null }
            relay.city.isBlank() -> null
            else -> countryName
        },
        flagCountry = relay.country,
        loadExitId = relay.exitId,
        hasActive = relay.active,
    )
}

/**
 * The recents worth a row, in the order they were used. A location the
 * catalogue dropped is skipped.
 */
internal fun recentPlaces(pins: List<ExitPin>, relays: List<WarrenRelaySummary>): List<Place> =
    pins.mapNotNull { resolvePlace(it, relays) }

/**
 * The whole exit-hop list, in render order. [byCountry] is the already filtered
 * and sorted catalogue; a non-empty [query] force-expands every branch so a
 * match is never hidden behind a collapsed parent.
 */
internal fun buildPickerRows(
    query: String,
    recents: List<Place>,
    customLists: List<CustomListSection>,
    byCountry: Map<String, Map<String, List<WarrenRelaySummary>>>,
    exitPin: ExitPin,
    expanded: ExpandedKeys,
): List<PickerRow> = buildList {
    val searching = query.isNotEmpty()
    val expandedCountries = expanded.countries
    val expandedCities = expanded.cities

    if (recents.isNotEmpty()) {
        add(PickerRow.RecentsHeader)
        recents.forEach { place -> add(savedRow(place, ExitSection.Recents, depth = 0, exitPin)) }
        add(PickerRow.Gap("gap-recents"))
    }

    // A list is created from a location's lists menu, and one left empty is
    // not listed, so the section appears only while a list holds something.
    if (customLists.isNotEmpty()) {
        add(PickerRow.CustomListsHeader)
        customLists.forEach { section -> addCustomListRows(section, searching, expanded.lists, exitPin) }
        add(PickerRow.Gap("gap-custom-lists"))
    }

    // The tree needs a title only to set it apart from a section above it.
    if (isNotEmpty()) add(PickerRow.AllLocationsHeader)
    // Automatic heads the catalogue rather than floating between sections: it
    // is the widest scope of "all locations", so it rounds into the same block
    // as the countries below it.
    if (!searching) {
        add(PickerRow.ExitAutomaticRow(isPinned = exitPin == ExitPin.Automatic))
    }
    byCountry.forEach { (country, cityMap) ->
        // A country with a single exit has nothing to expand: its row selects that exit.
        val onlyExit = cityMap.values.singleOrNull()?.singleOrNull()
        if (onlyExit != null) {
            add(singleExitCountryRow(country, onlyExit, exitPin))
            return@forEach
        }
        val countryExpanded = searching || country in expandedCountries
        add(
            PickerRow.CountryHeader(
                country = country,
                display = countryDisplayName(country),
                expanded = countryExpanded,
                hasActive = cityMap.values.any { rs -> rs.any { it.active } },
                isPinned = exitPin.pinsCountry(country),
            )
        )
        if (countryExpanded) {
            cityMap.forEach { (city, cityRelays) ->
                addCityRows(country, city, cityRelays, exitPin, searching, expandedCities)
            }
            // Only an expanded country breaks the block: collapsed neighbours
            // stay a contiguous list instead of a stack of isolated pills.
            add(PickerRow.Gap("gap-country-$country"))
        }
    }
}

private fun savedRow(place: Place, section: ExitSection, depth: Int, exitPin: ExitPin) =
    PickerRow.SavedRow(
        place = place,
        section = section,
        depth = depth,
        isPinned = place.location.sameLocationAs(exitPin),
    )

private fun MutableList<PickerRow>.addCustomListRows(
    section: CustomListSection,
    searching: Boolean,
    expandedLists: Set<String>,
    exitPin: ExitPin,
) {
    // A search opens every list it keeps, as it opens every country.
    val open = searching || section.name in expandedLists
    add(PickerRow.CustomListRow(name = section.name, count = section.places.size, expanded = open))
    if (!open) return
    val custom = ExitSection.Custom(section.name)
    section.places.forEach { place -> add(savedRow(place, custom, depth = 1, exitPin)) }
    add(PickerRow.Gap("gap-list-${section.name}"))
}

private fun singleExitCountryRow(
    country: String,
    relay: WarrenRelaySummary,
    exitPin: ExitPin,
): PickerRow.ExitRow =
    PickerRow.ExitRow(
        relay = relay,
        title = countryDisplayName(country),
        ordinal = null,
        depth = 0,
        isPinned = exitPin == ExitPin.Exit(relay.exitId) ||
            exitPin.pinsCountry(country) ||
            exitPin.pinsCity(country, relay.city),
        subtitle = relay.city.ifBlank { null },
    )

private fun MutableList<PickerRow>.addCityRows(
    country: String,
    city: String,
    cityRelays: List<WarrenRelaySummary>,
    exitPin: ExitPin,
    searching: Boolean,
    expandedCities: Set<String>,
) {
    val label = city.ifBlank { countryDisplayName(country) }
    if (cityRelays.size == 1) {
        val relay = cityRelays.first()
        add(
            PickerRow.ExitRow(
                relay = relay,
                title = label,
                ordinal = null,
                depth = 1,
                isPinned = exitPin == ExitPin.Exit(relay.exitId) ||
                    exitPin.pinsCity(country, city),
            )
        )
        return
    }

    val cityExpanded = searching || cityKey(country, city) in expandedCities
    add(
        PickerRow.CityHeader(
            country = country,
            city = city,
            expanded = cityExpanded,
            hasActive = cityRelays.any { it.active },
            isPinned = exitPin.pinsCity(country, city),
        )
    )
    if (!cityExpanded) return

    // Ordinals are derived from the sorted exit id so the same node keeps the
    // same number across refreshes; the endpoint address is never a label.
    cityRelays.sortedBy { it.exitId }.forEachIndexed { i, relay ->
        add(
            PickerRow.ExitRow(
                relay = relay,
                title = label,
                ordinal = i + 1,
                depth = 2,
                isPinned = exitPin == ExitPin.Exit(relay.exitId),
            )
        )
    }
}

/**
 * Row to scroll to when the picker opens, or -1 when the selection is not on
 * screen yet. The enclosing country header is preferred over the pinned row
 * itself so the selection lands with its parent context visible rather than
 * flush against the top edge.
 *
 * Recents and custom lists ([PickerRow.SavedRow]) are never a target: they duplicate the
 * pinned exit above the tree, and landing on the duplicate would burn the
 * one-shot scroll before the tree branch has even been expanded.
 */
internal fun scrollTargetIndex(rows: List<PickerRow>): Int {
    val pinned = rows.indexOfFirst { row ->
        when (row) {
            is PickerRow.CountryHeader -> row.isPinned
            is PickerRow.CityHeader -> row.isPinned
            is PickerRow.ExitRow -> row.isPinned
            else -> false
        }
    }
    if (pinned < 0) {
        return rows.indexOfFirst {
            it.isPinned &&
                (
                    it is PickerRow.ExitAutomaticRow ||
                        it is PickerRow.EntryAutomaticRow ||
                        it is PickerRow.EntryCountryRow
                    )
        }
    }
    // A country holding a single exit is its own row, with no header above it.
    val standsForCountry = (rows[pinned] as? PickerRow.ExitRow)?.depth == 0
    val header = (pinned downTo 0).firstOrNull { rows[it] is PickerRow.CountryHeader }
    return if (standsForCountry || header == null) pinned else header
}

/**
 * Whether the one-shot scroll-to-selection is worth performing. A selection
 * already on screen is left alone: scrolling to it anyway pushes the sections
 * above it (recents, custom lists) out of the viewport for no gain, and on a
 * list barely taller than the screen it clamps to the end and hides them for
 * good.
 */
internal fun shouldScrollTo(target: Int, firstVisible: Int, lastVisible: Int): Boolean =
    target >= 0 && (target < firstVisible || target > lastVisible)

/** Index of the last row belonging to the country whose header sits at [headerIndex]. */
internal fun countryBlockEnd(rows: List<PickerRow>, headerIndex: Int): Int {
    var k = headerIndex + 1
    while (k < rows.size && rows[k] !is PickerRow.Gap && rows[k] !is PickerRow.CountryHeader) k++
    return k - 1
}
