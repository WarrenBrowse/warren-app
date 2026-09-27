package com.warrenbrowse.vpn.feature.settings.impl

import com.warrenbrowse.vpn.lib.model.countryDisplayName
import com.warrenbrowse.vpn.lib.repository.ExitPin
import com.warrenbrowse.vpn.lib.repository.WarrenRelaySummary
import com.warrenbrowse.vpn.lib.ui.designsystem.Position
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue
import org.junit.jupiter.api.Test

/**
 * The picker's row builder. It is the whole visual structure of the screen
 * (sections, block rounding, indentation depth, labels), so the rules the
 * desktop enforces by construction are pinned here instead of by inspection.
 */
class LocationPickerRowsTest {

    private fun relay(
        exitId: String,
        country: String,
        city: String,
        active: Boolean = true,
    ) = WarrenRelaySummary(
        exitId = exitId,
        exitPubkeyHex = "aa",
        endpoint = "10.0.0.1:443",
        country = country,
        city = city,
        active = active,
        weight = 1,
    )

    private fun byCountry(
        relays: List<WarrenRelaySummary>
    ): Map<String, Map<String, List<WarrenRelaySummary>>> =
        relays.groupBy { it.country }.mapValues { (_, rs) -> rs.groupBy { it.city } }

    private fun rows(
        relays: List<WarrenRelaySummary>,
        query: String = "",
        recents: List<WarrenRelaySummary> = emptyList(),
        recentScopes: List<ExitPin> = emptyList(),
        customLists: Map<String, List<ExitPin>> = emptyMap(),
        exitPin: ExitPin = ExitPin.Automatic,
        expandedCountries: Set<String> = emptySet(),
        expandedCities: Set<String> = emptySet(),
        expandedLists: Set<String> = emptySet(),
    ) = buildPickerRows(
        query = query,
        recents = recentPlaces(recents.map { ExitPin.Exit(it.exitId) } + recentScopes, relays),
        customLists = visibleCustomLists(customLists, relays, query),
        byCountry = byCountry(relays),
        exitPin = exitPin,
        expanded = ExpandedKeys(expandedCountries, expandedCities, expandedLists),
    )

    private fun List<PickerRow>.saved(section: ExitSection) =
        filterIsInstance<PickerRow.SavedRow>().filter { it.section == section }

    private val catalogue = listOf(
        relay("de1", "DE", "Frankfurt"),
        relay("de2", "DE", "Frankfurt"),
        relay("fr1", "FR", "Paris"),
        relay("se1", "SE", "Malmo"),
    )

    // Search threshold

    @Test
    fun `the row function lists the entry countries once each behind the automatic row`() {
        val rows =
            pickerRows(
                PickerInputs(
                    relays = listOf(relay("a", "nl", "Amsterdam"), relay("b", "nl", "Rotterdam"), relay("c", "de", "Berlin")),
                    query = "",
                    scope = PickerScope.Entry,
                    entryCountry = "de",
                    recentsEnabled = true,
                    recentPins = emptyList(),
                    customLists = emptyMap(),
                    exitPin = ExitPin.Automatic,
                    expanded = ExpandedKeys(emptySet(), emptySet()),
                )
            )

        assertTrue(rows.first() is PickerRow.EntryAutomaticRow)
        val countries = rows.filterIsInstance<PickerRow.EntryCountryRow>()
        assertEquals(listOf("de", "nl"), countries.map { it.country })
        assertTrue(countries.single { it.country == "de" }.isPinned)
    }

    @Test
    fun `the row function keeps recents out of a search`() {
        val relays = listOf(relay("a", "nl", "Amsterdam"), relay("c", "de", "Berlin"))
        fun rowsFor(query: String) =
            pickerRows(
                PickerInputs(
                    relays = relays,
                    query = query,
                    scope = PickerScope.Exit,
                    entryCountry = null,
                    recentsEnabled = true,
                    recentPins = listOf(ExitPin.Exit("a")),
                    customLists = emptyMap(),
                    exitPin = ExitPin.Automatic,
                    expanded = ExpandedKeys(emptySet(), emptySet()),
                )
            )

        assertTrue(rowsFor("").any { it is PickerRow.RecentsHeader })
        assertTrue(rowsFor("ber").none { it is PickerRow.RecentsHeader })
    }

    @Test
    fun `a one character query is not applied`() {
        assertEquals("", appliedQuery("f"))
        assertEquals("", appliedQuery("  f  "))
    }

    @Test
    fun `a two character query is applied trimmed`() {
        assertEquals("fr", appliedQuery("  fr "))
    }

    // Labels

    @Test
    fun `exits sharing a city are numbered from the sorted exit id`() {
        val built = rows(
            catalogue,
            expandedCountries = setOf("DE"),
            expandedCities = setOf(cityKey("DE", "Frankfurt")),
        )
        val exits = built.filterIsInstance<PickerRow.ExitRow>().filter { it.relay.country == "DE" }

        assertEquals(listOf("de1", "de2"), exits.map { it.relay.exitId })
        assertEquals(listOf(1, 2), exits.map { it.ordinal })
        assertTrue(exits.all { it.title == "Frankfurt" })
    }

    @Test
    fun `no row label carries a raw endpoint`() {
        val built = rows(
            catalogue,
            expandedCountries = setOf("DE", "FR", "SE"),
            expandedCities = setOf(cityKey("DE", "Frankfurt")),
        )

        assertTrue(built.filterIsInstance<PickerRow.ExitRow>().none { it.title.contains("10.0.0") })
    }

    @Test
    fun `a lone exit in a city keeps the city name and no ordinal`() {
        val built = rows(
            catalogue + relay("de3", "DE", "Berlin"),
            expandedCountries = setOf("DE"),
        )
        val berlin = built.filterIsInstance<PickerRow.ExitRow>().single { it.relay.exitId == "de3" }

        assertEquals("Berlin", berlin.title)
        assertEquals(null, berlin.ordinal)
        assertEquals(1, berlin.depth)
    }

    // Structure

    @Test
    fun `the country tree is introduced by an all locations header under recents`() {
        val built = rows(catalogue, recents = listOf(catalogue[0]))
        val header = built.indexOfFirst { it is PickerRow.AllLocationsHeader }
        val firstCountry = built.indexOfFirst { it is PickerRow.CountryHeader }

        assertTrue(header >= 0)
        assertTrue(header < firstCountry)
    }

    @Test
    fun `the country tree is introduced by an all locations header under custom lists`() {
        val built = rows(catalogue, customLists = mapOf("Work" to listOf(ExitPin.Exit("fr1"))))

        assertTrue(built.any { it is PickerRow.AllLocationsHeader })
    }

    @Test
    fun `the country tree alone needs no header`() {
        val built = rows(catalogue)

        assertTrue(built.none { it is PickerRow.AllLocationsHeader })
        assertTrue(built.first() is PickerRow.ExitAutomaticRow)
    }

    @Test
    fun `a country holding a single exit is one row that selects it, with no expand toggle`() {
        val built = rows(catalogue)
        val france = built.filterIsInstance<PickerRow.ExitRow>().single { it.relay.exitId == "fr1" }
        val germany = built.filterIsInstance<PickerRow.CountryHeader>().single()

        assertEquals("DE", germany.country)
        assertTrue(germany.expandable)
        assertFalse(france.expandable)
        assertEquals(countryDisplayName("FR"), france.title)
        assertEquals("Paris", france.subtitle)
        assertEquals(0, france.depth)
        assertEquals("FR", france.flagCountry)
        assertEquals(ExitPin.Exit("fr1"), france.location)
    }

    @Test
    fun `a single-exit country stays one row when marked expanded or searched`() {
        val expanded = rows(catalogue, expandedCountries = setOf("FR"))
        val searched = rows(catalogue, query = "paris")

        for (built in listOf(expanded, searched)) {
            assertEquals(1, built.count { it is PickerRow.ExitRow && it.relay.exitId == "fr1" })
            assertTrue(built.none { it is PickerRow.CountryHeader && it.country == "FR" })
            assertTrue(built.none { it.key == "gap-country-FR" })
        }
    }

    @Test
    fun `a single-exit country row carries a country, city or exit pin`() {
        fun pinned(pin: ExitPin) =
            rows(catalogue, exitPin = pin)
                .filterIsInstance<PickerRow.ExitRow>()
                .single { it.relay.exitId == "fr1" }
                .isPinned

        assertTrue(pinned(ExitPin.Exit("fr1")))
        assertTrue(pinned(ExitPin.Country("FR")))
        assertTrue(pinned(ExitPin.City("FR", "Paris")))
        assertFalse(pinned(ExitPin.Country("DE")))
    }

    @Test
    fun `scroll targets a pinned single-exit country row itself`() {
        val built = assignPositions(rows(catalogue, exitPin = ExitPin.Country("SE")))

        val target = built[scrollTargetIndex(built)]

        assertTrue(target is PickerRow.ExitRow && target.relay.exitId == "se1")
    }

    @Test
    fun `only tree rows at the top level lead with a flag`() {
        val built = rows(
            catalogue,
            expandedCountries = setOf("DE"),
            expandedCities = setOf(cityKey("DE", "Frankfurt")),
        )

        assertEquals("FR", built.filterIsInstance<PickerRow.ExitRow>().single { it.relay.exitId == "fr1" }.flagCountry)
        assertTrue(
            built.filterIsInstance<PickerRow.ExitRow>().filter { it.relay.country == "DE" }
                .all { it.flagCountry == null }
        )
    }

    @Test
    fun `a gap follows an expanded country only`() {
        val collapsed = rows(catalogue)
        assertTrue(collapsed.none { it is PickerRow.Gap && it.key.startsWith("gap-country") })

        val expanded = rows(catalogue, expandedCountries = setOf("DE"))
        assertEquals(
            listOf("gap-country-DE"),
            expanded.filterIsInstance<PickerRow.Gap>()
                .map { it.key }
                .filter { it.startsWith("gap-country") },
        )
    }

    @Test
    fun `automatic heads the all locations section`() {
        val built = rows(catalogue, recents = listOf(catalogue[0]))
        val header = built.indexOfFirst { it is PickerRow.AllLocationsHeader }

        assertTrue(built[header + 1] is PickerRow.ExitAutomaticRow)
        assertTrue(built[header + 2] is PickerRow.CountryHeader)
    }

    @Test
    fun `automatic rounds into the same block as the collapsed countries`() {
        val built = assignPositions(rows(catalogue))

        assertEquals(4, built.size)
        assertEquals(
            listOf(Position.Top, Position.Middle, Position.Middle, Position.Bottom),
            built.map { it.position },
        )
    }

    @Test
    fun `recent rows are introduced by a recents header`() {
        val built = rows(catalogue, recents = listOf(catalogue[0], catalogue[2]))

        assertEquals(0, built.indexOfFirst { it is PickerRow.RecentsHeader })
        assertTrue(built[1] is PickerRow.SavedRow)
        assertEquals(2, built.saved(ExitSection.Recents).size)
    }

    @Test
    fun `the pinned exit stays listed in recents`() {
        val built = rows(catalogue, recents = catalogue, exitPin = ExitPin.Exit("de1"))
        val recents = built.saved(ExitSection.Recents)

        assertEquals(catalogue.map { ExitPin.Exit(it.exitId) }, recents.map { it.place.location })
        assertEquals(listOf(true, false, false, false), recents.map { it.isPinned })
    }

    @Test
    fun `a selection already on screen is not scrolled to`() {
        assertFalse(shouldScrollTo(target = 3, firstVisible = 0, lastVisible = 8))
        assertTrue(shouldScrollTo(target = 12, firstVisible = 0, lastVisible = 8))
        assertTrue(shouldScrollTo(target = 1, firstVisible = 4, lastVisible = 9))
        assertFalse(shouldScrollTo(target = -1, firstVisible = 0, lastVisible = 8))
    }

    @Test
    fun `an exit under a city header sits one depth below it`() {
        val built = rows(
            catalogue,
            expandedCountries = setOf("DE"),
            expandedCities = setOf(cityKey("DE", "Frankfurt")),
        )
        val city = built.filterIsInstance<PickerRow.CityHeader>().single()
        val exit = built.filterIsInstance<PickerRow.ExitRow>().first { it.relay.country == "DE" }

        assertEquals(1, city.depth)
        assertEquals(2, exit.depth)
    }

    @Test
    fun `the recents toggle is no longer a row`() {
        val built = rows(catalogue, recents = listOf(catalogue[0]))

        assertTrue(built.none { it.key == "recents-toggle" })
    }

    @Test
    fun `a recent country or city is listed at its own depth in recents`() {
        // Desktop recents are locations at the depth they were picked
        // (`RecentGeographicalLocation`), so a country or city pin is a recent
        // row of its own, its parent country under the name like the desktop row.
        val built = rows(
            catalogue,
            recentScopes = listOf(ExitPin.City("DE", "Frankfurt"), ExitPin.Country("DE")),
            exitPin = ExitPin.Country("DE"),
        )
        val recents = built.saved(ExitSection.Recents)

        assertEquals(
            listOf(ExitPin.City("DE", "Frankfurt"), ExitPin.Country("DE")),
            recents.map { it.place.location },
        )
        assertEquals(listOf(false, true), recents.map { it.isPinned })
        // Country names follow the device locale, so the expectation does too.
        assertEquals("Frankfurt", recents[0].place.title)
        assertEquals(countryDisplayName("DE"), recents[0].place.subtitle)
        assertEquals(countryDisplayName("DE"), recents[1].place.title)
        assertEquals(null, recents[1].place.subtitle)
        assertEquals(0, built.indexOfFirst { it is PickerRow.RecentsHeader })
    }

    @Test
    fun `recents keep the order they were used in across depths`() {
        val places = recentPlaces(
            listOf(ExitPin.Country("SE"), ExitPin.Exit("de1"), ExitPin.City("FR", "Paris")),
            catalogue,
        )

        assertEquals(
            listOf(ExitPin.Country("SE"), ExitPin.Exit("de1"), ExitPin.City("FR", "Paris")),
            places.map { it.location },
        )
    }

    @Test
    fun `a recent the catalogue no longer serves is dropped`() {
        val places = recentPlaces(
            listOf(ExitPin.Country("ZZ"), ExitPin.City("DE", "Munich"), ExitPin.Exit("gone")),
            catalogue,
        )
        assertTrue(places.isEmpty())
    }

    @Test
    fun `a recent scope with every exit down stays listed but inactive`() {
        val down = catalogue.map { if (it.country == "SE") it.copy(active = false) else it }
        val place = recentPlaces(listOf(ExitPin.Country("SE")), down).single()
        assertFalse(place.hasActive)
    }

    // Row anatomy: one shape for every saved location

    @Test
    fun `a saved exit leads with its country flag, names its country and shows its load`() {
        val place = resolvePlace(ExitPin.Exit("de2"), catalogue)!!

        assertEquals("DE", place.flagCountry)
        assertEquals("Frankfurt", place.title)
        assertEquals(2, place.ordinal)
        assertEquals(countryDisplayName("DE"), place.subtitle)
        assertEquals("de2", place.loadExitId)
    }

    @Test
    fun `a saved exit alone in its country reads like its tree row`() {
        val place = resolvePlace(ExitPin.Exit("fr1"), catalogue)!!

        assertEquals(countryDisplayName("FR"), place.title)
        assertEquals("Paris", place.subtitle)
        assertEquals(null, place.ordinal)
        assertEquals("fr1", place.loadExitId)
    }

    @Test
    fun `a saved city shows a load only when it holds exactly one exit`() {
        val frankfurt = resolvePlace(ExitPin.City("DE", "Frankfurt"), catalogue)!!
        val paris = resolvePlace(ExitPin.City("FR", "Paris"), catalogue)!!

        assertEquals("DE", frankfurt.flagCountry)
        assertEquals(null, frankfurt.loadExitId)
        assertEquals("fr1", paris.loadExitId)
        assertEquals(countryDisplayName("FR"), paris.subtitle)
    }

    @Test
    fun `a saved country shows a load only when it holds exactly one exit`() {
        assertEquals(null, resolvePlace(ExitPin.Country("DE"), catalogue)!!.loadExitId)
        assertEquals("se1", resolvePlace(ExitPin.Country("SE"), catalogue)!!.loadExitId)
    }

    @Test
    fun `automatic is not a place`() {
        assertEquals(null, resolvePlace(ExitPin.Automatic, catalogue))
    }

    @Test
    fun `every tree row names the location its menu acts on`() {
        val built = rows(
            catalogue,
            expandedCountries = setOf("DE"),
            expandedCities = setOf(cityKey("DE", "Frankfurt")),
        )

        assertEquals(
            ExitPin.Country("DE"),
            built.filterIsInstance<PickerRow.CountryHeader>().single().location,
        )
        assertEquals(
            ExitPin.City("DE", "Frankfurt"),
            built.filterIsInstance<PickerRow.CityHeader>().single().location,
        )
        assertEquals(
            ExitPin.Exit("de2"),
            built.filterIsInstance<PickerRow.ExitRow>().single { it.relay.exitId == "de2" }.location,
        )
    }

    // Selection

    @Test
    fun `a saved row carrying the pinned location renders as selected`() {
        val built = rows(
            catalogue,
            recents = listOf(catalogue[0]),
            customLists = mapOf("Work" to listOf(ExitPin.City("DE", "Frankfurt"))),
            exitPin = ExitPin.City("de", "frankfurt"),
            expandedLists = setOf("Work"),
        )

        assertFalse(built.saved(ExitSection.Recents).single().isPinned)
        assertTrue(built.saved(ExitSection.Custom("Work")).single().isPinned)
    }

    @Test
    fun `scroll targets the country header enclosing the pinned exit`() {
        val built = assignPositions(
            rows(
                catalogue,
                recents = listOf(catalogue[0]),
                exitPin = ExitPin.Exit("de1"),
                expandedCountries = setOf("DE"),
                expandedCities = setOf(cityKey("DE", "Frankfurt")),
            )
        )

        val target = scrollTargetIndex(built)

        assertTrue(built[target] is PickerRow.CountryHeader)
        assertEquals("DE", (built[target] as PickerRow.CountryHeader).country)
    }

    @Test
    fun `a collapsed tree never scrolls to the recents duplicate`() {
        val built = assignPositions(
            rows(catalogue, recents = listOf(catalogue[0]), exitPin = ExitPin.Exit("de1"))
        )

        assertEquals(-1, scrollTargetIndex(built))
    }

    @Test
    fun `the country block ends before the next country`() {
        val built = rows(catalogue, expandedCountries = setOf("DE"))
        val header = built.indexOfFirst {
            it is PickerRow.CountryHeader && it.country == "DE"
        }

        val end = countryBlockEnd(built, header)

        assertTrue(end > header)
        assertTrue(built[end] !is PickerRow.CountryHeader)
        assertTrue(built.getOrNull(end + 1) is PickerRow.Gap)
    }

    // Custom lists

    @Test
    fun `the custom lists section is hidden while there is no list`() {
        val built = rows(catalogue)

        assertTrue(built.none { it is PickerRow.CustomListsHeader })
        assertTrue(built.none { it.key == "gap-custom-lists" })
    }

    @Test
    fun `the custom lists section is hidden while every list is empty`() {
        val built = rows(
            catalogue,
            customLists = mapOf("Empty" to emptyList(), "Gone" to listOf(ExitPin.Exit("gone"))),
        )

        assertTrue(built.none { it is PickerRow.CustomListsHeader })
        assertTrue(built.none { it is PickerRow.CustomListRow })
        assertTrue(built.none { it is PickerRow.AllLocationsHeader })
    }

    @Test
    fun `an empty list is left out of a section other lists keep open`() {
        val built = rows(
            catalogue,
            customLists = mapOf("Empty" to emptyList(), "Work" to listOf(ExitPin.Country("SE"))),
        )

        assertEquals(listOf("Work"), built.filterIsInstance<PickerRow.CustomListRow>().map { it.name })
    }

    @Test
    fun `a list row counts its locations and folds its members until expanded`() {
        val lists = mapOf("Work" to listOf(ExitPin.Country("SE"), ExitPin.Exit("de1")))
        val collapsed = rows(catalogue, customLists = lists)
        val expanded = rows(catalogue, customLists = lists, expandedLists = setOf("Work"))

        val row = collapsed.filterIsInstance<PickerRow.CustomListRow>().single()
        assertEquals(2, row.count)
        assertFalse(row.expanded)
        assertTrue(collapsed.saved(ExitSection.Custom("Work")).isEmpty())

        val members = expanded.saved(ExitSection.Custom("Work"))
        assertTrue(expanded.filterIsInstance<PickerRow.CustomListRow>().single().expanded)
        assertEquals(listOf(ExitPin.Country("SE"), ExitPin.Exit("de1")), members.map { it.place.location })
        assertTrue(members.all { it.depth == 1 })
    }

    @Test
    fun `a search opens every list it keeps`() {
        val built = rows(
            catalogue,
            query = "paris",
            customLists = mapOf("Work" to listOf(ExitPin.Exit("fr1"), ExitPin.Country("SE"))),
        )

        assertTrue(built.filterIsInstance<PickerRow.CustomListRow>().single().expanded)
        assertEquals(
            listOf(ExitPin.Exit("fr1")),
            built.saved(ExitSection.Custom("Work")).map { it.place.location },
        )
    }

    @Test
    fun `a custom list survives a search matching its name`() {
        val lists = visibleCustomLists(mapOf("Nordics" to listOf(ExitPin.Exit("se1"))), catalogue, "nor")

        assertEquals(listOf("Nordics"), lists.map { it.name })
        assertEquals(listOf(ExitPin.Exit("se1")), lists.single().places.map { it.location })
    }

    @Test
    fun `a custom list survives a search matching one of its members`() {
        val lists = visibleCustomLists(
            mapOf("Work" to listOf(ExitPin.Exit("se1"), ExitPin.City("FR", "Paris"))),
            catalogue,
            "paris",
        )

        assertEquals(listOf(ExitPin.City("FR", "Paris")), lists.single().places.map { it.location })
    }

    @Test
    fun `a custom list matching nothing is dropped from a search`() {
        val lists = visibleCustomLists(mapOf("Work" to listOf(ExitPin.Exit("se1"))), catalogue, "paris")

        assertTrue(lists.isEmpty())
    }

    // The lists menu

    @Test
    fun `the lists menu offers every list, empty ones included, checking those holding the location`() {
        val lists = mapOf(
            "Empty" to emptyList(),
            "Work" to listOf(ExitPin.City("DE", "Frankfurt")),
            "Travel" to listOf(ExitPin.Country("FR")),
        )

        val menu = listMemberships(ExitPin.City("de", "frankfurt"), lists)

        assertEquals(
            listOf(
                ListMembership("Empty", contains = false),
                ListMembership("Travel", contains = false),
                ListMembership("Work", contains = true),
            ),
            menu,
        )
    }

    @Test
    fun `toggling a list the location is not in adds it`() {
        val added = mutableListOf<Pair<String, ExitPin>>()

        val outcome = toggleListMembership(
            name = "Work",
            location = ExitPin.Country("SE"),
            customLists = mapOf("Work" to listOf(ExitPin.Exit("de1"))),
            add = { name, pin -> added += name to pin },
            remove = { _, _ -> error("nothing to remove") },
        )

        assertEquals(ListToggle.Added, outcome)
        assertEquals(listOf<Pair<String, ExitPin>>("Work" to ExitPin.Country("SE")), added)
    }

    @Test
    fun `toggling a list the location is in removes the stored entry`() {
        val removed = mutableListOf<Pair<String, ExitPin>>()

        val outcome = toggleListMembership(
            name = "Work",
            location = ExitPin.City("de", "frankfurt"),
            customLists = mapOf("Work" to listOf(ExitPin.City("DE", "Frankfurt"))),
            add = { _, _ -> error("nothing to add") },
            remove = { name, pin -> removed += name to pin },
        )

        assertEquals(ListToggle.Removed, outcome)
        assertEquals(listOf<Pair<String, ExitPin>>("Work" to ExitPin.City("DE", "Frankfurt")), removed)
    }

    // Filtering

    @Test
    fun `a search matches the localized country name as well as the code`() {
        assertTrue(relayMatches(relay("de1", "DE", "Frankfurt"), "de"))
        assertTrue(relayMatches(relay("de1", "DE", "Frankfurt"), "frankfurt"))
        assertFalse(relayMatches(relay("de1", "DE", "Frankfurt"), "zzz"))
    }

    @Test
    fun `a search never matches the raw endpoint`() {
        assertFalse(relayMatches(relay("de1", "DE", "Frankfurt"), "10.0.0.1"))
    }
}
