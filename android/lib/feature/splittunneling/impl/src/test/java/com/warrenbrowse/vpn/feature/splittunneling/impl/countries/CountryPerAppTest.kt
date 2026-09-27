package com.warrenbrowse.vpn.feature.splittunneling.impl.countries

import kotlin.test.assertEquals
import com.warrenbrowse.vpn.feature.splittunneling.impl.applist.AppData
import com.warrenbrowse.vpn.lib.model.AppExit
import com.warrenbrowse.vpn.lib.model.AppRouteLine
import com.warrenbrowse.vpn.lib.model.AppRouteUnavailableReason
import com.warrenbrowse.vpn.lib.model.PackageName
import com.warrenbrowse.vpn.lib.repository.WarrenRelaySummary
import org.junit.jupiter.api.Test

class CountryPerAppTest {

    private val names =
        mapOf("se" to "Sweden", "de" to "Germany", "fi" to "Finland", "ch" to "Switzerland")
    private val countryName = { code: String -> names[code] ?: code }

    @Test
    fun `only countries and cities with an active server are offered, by name`() {
        val relays =
            listOf(
                relay("se", "Stockholm"),
                relay("se", "Malmo"),
                relay("se", "Stockholm"),
                relay("de", "Berlin", active = false),
                relay("DE", "Frankfurt"),
                relay("fi", "Helsinki", active = false),
            )

        val options = buildCountryOptions(relays, "", countryName)

        assertEquals(
            listOf(
                CountryOption("de", "Germany", listOf("Frankfurt")),
                CountryOption("se", "Sweden", listOf("Malmo", "Stockholm")),
            ),
            options,
        )
    }

    @Test
    fun `a search matching a country keeps all its cities`() {
        val relays = listOf(relay("se", "Stockholm"), relay("se", "Malmo"), relay("de", "Berlin"))

        val options = buildCountryOptions(relays, " swe", countryName)

        assertEquals(listOf(CountryOption("se", "Sweden", listOf("Malmo", "Stockholm"))), options)
    }

    @Test
    fun `a search matching only cities keeps those cities`() {
        val relays =
            listOf(
                relay("se", "Stockholm"),
                relay("se", "Malmo"),
                relay("de", "Berlin"),
                relay("ch", "Zurich"),
            )

        val options = buildCountryOptions(relays, "HOLM", countryName)

        assertEquals(listOf(CountryOption("se", "Sweden", listOf("Stockholm"))), options)
    }

    @Test
    fun `the picker marks the current exit and every exit in use`() {
        val app = AppData(PackageName("org.chat"), 0, "Chat")
        val exits = mapOf("org.chat" to AppExit("se"), "org.maps" to AppExit("de", "Berlin"))

        val picker =
            countryPicker(app, exits, listOf(relay("se", "Stockholm")), "", setOf("se"), countryName)

        assertEquals(AppExit("se"), picker.current)
        assertEquals(setOf(AppExit("se"), AppExit("de", "Berlin")), picker.exitsInUse)
        assertEquals(setOf("se"), picker.expanded)
    }

    @Test
    fun `a picker search shows the cities of every country it kept`() {
        val app = AppData(PackageName("org.chat"), 0, "Chat")
        val relays = listOf(relay("se", "Stockholm"), relay("de", "Berlin"))

        val picker = countryPicker(app, emptyMap(), relays, "o", emptySet(), countryName)

        assertEquals(setOf("se"), picker.expanded)
    }

    @Test
    fun `the chip reads the country, or the city then the country`() {
        assertEquals("Sweden", exitLabel(AppExit("se"), countryName))
        assertEquals("Berlin, Germany", exitLabel(AppExit("de", "Berlin"), countryName))
    }

    @Test
    fun `only a route that cannot run for a reason of its own is red`() {
        assertEquals(RouteTone.Positive, AppRouteLine.Connected(null).tone())
        assertEquals(RouteTone.Pending, AppRouteLine.Waiting.tone())
        assertEquals(RouteTone.Pending, AppRouteLine.Connecting.tone())
        assertEquals(
            RouteTone.Pending,
            AppRouteLine.Unavailable(AppRouteUnavailableReason.TunnelDown).tone(),
        )
        assertEquals(
            RouteTone.Error,
            AppRouteLine.Unavailable(AppRouteUnavailableReason.WaitingForRoute).tone(),
        )
        assertEquals(RouteTone.Error, AppRouteLine.Unavailable(null).tone())
        assertEquals(RouteTone.Muted, AppRouteLine.Paused.tone())
        assertEquals(RouteTone.Muted, AppRouteLine.Bypassed.tone())
    }

    @Test
    fun `an address is isolated left to right inside a sentence`() {
        assertEquals("\u2066198.51.100.7\u2069", isolateLeftToRight("198.51.100.7"))
    }

    private fun relay(country: String, city: String, active: Boolean = true) =
        WarrenRelaySummary(
            exitId = "$country-$city",
            exitPubkeyHex = "",
            endpoint = "",
            country = country,
            city = city,
            active = active,
            weight = 1,
        )
}
