package com.warrenbrowse.vpn.lib.model

import java.util.Locale
import kotlin.test.assertEquals
import kotlin.test.assertNotEquals
import kotlin.test.assertTrue
import org.junit.jupiter.api.Test

class CountryNamesTest {
    @Test
    fun `maps an ISO alpha-2 code to its English display name`() {
        assertEquals("Germany", countryDisplayName("de", Locale.ENGLISH))
    }

    @Test
    fun `is case insensitive on the code`() {
        assertEquals("Netherlands", countryDisplayName("NL", Locale.ENGLISH))
    }

    @Test
    fun `localizes the display name to the UI locale`() {
        // The whole point of the mapping: the same code renders in the user's
        // language, not the raw wire code.
        val french = countryDisplayName("de", Locale.FRENCH)
        assertNotEquals("de", french)
        assertNotEquals(countryDisplayName("de", Locale.ENGLISH), french)
    }

    @Test
    fun `returns a non-code input unchanged (already a display name)`() {
        assertEquals("Frankfurt", countryDisplayName("Frankfurt", Locale.ENGLISH))
    }

    @Test
    fun `returns blank input unchanged`() {
        assertEquals("", countryDisplayName("", Locale.ENGLISH))
    }

    @Test
    fun `falls back to the raw code for an unknown region`() {
        // ZZ is a reserved, unassigned region: no display name exists, so the
        // caller still gets something to show rather than an empty string.
        assertTrue(countryDisplayName("ZZ", Locale.ENGLISH).isNotBlank())
    }

    // ICU's lookup is called inside the picker's sort comparator and once per
    // relay per keystroke, on the main thread, so the answer is kept per code
    // and locale.

    @Test
    fun `a code is looked up once per locale and served from the memo after`() {
        CountryNames.clear()

        countryDisplayName("nl", Locale.ENGLISH)
        countryDisplayName("nl", Locale.ENGLISH)
        countryDisplayName("NL", Locale.ENGLISH)

        assertEquals(1, CountryNames.size(), "one entry per code and locale")
        assertEquals("Netherlands", countryDisplayName("nl", Locale.ENGLISH))
    }

    @Test
    fun `the memo is keyed on the locale, so a language change is a fresh lookup`() {
        CountryNames.clear()

        countryDisplayName("de", Locale.ENGLISH)
        countryDisplayName("de", Locale.FRENCH)

        assertEquals(2, CountryNames.size())
    }

    @Test
    fun `anything but a two letter code is never memoised`() {
        CountryNames.clear()

        countryDisplayName("Netherlands")
        countryDisplayName("  ")

        assertEquals(0, CountryNames.size())
    }
}
