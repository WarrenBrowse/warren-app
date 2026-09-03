package com.warrenbrowse.vpn.feature.settings.impl

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
        customLists: List<CustomListSection> = emptyList(),
        exitPin: ExitPin = ExitPin.Automatic,
        expandedCountries: Set<String> = emptySet(),
        expandedCities: Set<String> = emptySet(),
    ) = buildPickerRows(
        query = query,
        recentRelays = recents,
        customLists = customLists,
        byCountry = byCountry(relays),
        exitPin = exitPin,
        expandedCountries = expandedCountries,
        expandedCities = expandedCities,
    )

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
                    recentExitIds = emptyList(),
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
                    recentExitIds = listOf("a"),
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
        val built = rows(catalogue, expandedCountries = setOf("FR"))
        val paris = built.filterIsInstance<PickerRow.ExitRow>().single { it.relay.exitId == "fr1" }

        assertEquals("Paris", paris.title)
        assertEquals(null, paris.ordinal)
    }

    // Structure

    @Test
    fun `the country tree is introduced by an all locations header`() {
        val built = rows(catalogue)
        val header = built.indexOfFirst { it is PickerRow.AllLocationsHeader }
        val firstCountry = built.indexOfFirst { it is PickerRow.CountryHeader }

        assertTrue(header >= 0)
        assertTrue(header < firstCountry)
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
        val built = rows(catalogue)
        val header = built.indexOfFirst { it is PickerRow.AllLocationsHeader }

        assertTrue(built[header + 1] is PickerRow.ExitAutomaticRow)
        assertTrue(built[header + 2] is PickerRow.CountryHeader)
    }

    @Test
    fun `automatic rounds into the same block as the collapsed countries`() {
        val built = assignPositions(rows(catalogue))
        val block = built.filter {
            it is PickerRow.ExitAutomaticRow || it is PickerRow.CountryHeader
        }

        assertEquals(4, block.size)
        assertEquals(
            listOf(Position.Top, Position.Middle, Position.Middle, Position.Bottom),
            block.map { it.position },
        )
    }

    @Test
    fun `recent rows are introduced by a recents header`() {
        val built = rows(catalogue, recents = listOf(catalogue[0], catalogue[2]))
        val header = built.indexOfFirst { it is PickerRow.RecentsHeader }
        val firstRecent = built.indexOfFirst {
            it is PickerRow.ExitRow && it.section == ExitSection.Recents
        }

        assertEquals(0, header)
        assertEquals(1, firstRecent)
    }

    @Test
    fun `the pinned exit stays listed in recents`() {
        val built = rows(catalogue, recents = catalogue, exitPin = ExitPin.Exit("de1"))
        val recents = built
            .filterIsInstance<PickerRow.ExitRow>()
            .filter { it.section == ExitSection.Recents }

        assertEquals(catalogue.map { it.exitId }, recents.map { it.relay.exitId })
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

    // Selection

    @Test
    fun `a recent row carrying the pinned exit renders as selected`() {
        val built = rows(catalogue, recents = listOf(catalogue[0]), exitPin = ExitPin.Exit("de1"))
        val recent = built
            .filterIsInstance<PickerRow.ExitRow>()
            .single { it.section == ExitSection.Recents }

        assertTrue(recent.isPinned)
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
    fun `the custom lists section is always announced with a hint when empty`() {
        val built = rows(catalogue)

        assertTrue(built.any { it is PickerRow.CustomListsHeader })
        assertTrue(built.any { it is PickerRow.CustomListsEmptyHint })
    }

    @Test
    fun `a custom list survives a search matching its name`() {
        val lists = visibleCustomLists(mapOf("Nordics" to listOf("se1")), catalogue, "nor")

        assertEquals(listOf("Nordics"), lists.map { it.name })
        assertEquals(listOf("se1"), lists.single().relays.map { it.exitId })
    }

    @Test
    fun `a custom list survives a search matching one of its members`() {
        val lists =
            visibleCustomLists(mapOf("Work" to listOf("se1", "fr1")), catalogue, "paris")

        assertEquals(listOf("fr1"), lists.single().relays.map { it.exitId })
    }

    @Test
    fun `a custom list matching nothing is dropped from a search`() {
        val lists = visibleCustomLists(mapOf("Work" to listOf("se1")), catalogue, "paris")

        assertTrue(lists.isEmpty())
    }

    @Test
    fun `the custom lists section disappears when a search matches no list`() {
        val built = rows(catalogue, query = "paris", customLists = emptyList())

        assertFalse(built.any { it is PickerRow.CustomListsHeader })
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
