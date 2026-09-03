package com.warrenbrowse.vpn.feature.home.impl.connect

import com.warrenbrowse.vpn.lib.ui.resource.R
import kotlin.test.assertEquals
import kotlin.test.assertNull
import org.junit.jupiter.api.Test

/**
 * What the connection card's flag slot draws for a country code: the desktop
 * flag artwork when the shared set has it, the emoji glyph when it does not,
 * nothing when the code is not a code (desktop CurrentCountryFlag).
 */
class CountryFlagTest {

    @Test
    fun `a country in the shared set draws the desktop flag artwork`() {
        assertEquals(
            CountryFlagSource.Artwork(R.drawable.flag_nl),
            countryFlagSource("nl"),
        )
    }

    @Test
    fun `the lookup is case and whitespace insensitive, as the wire is`() {
        assertEquals(
            CountryFlagSource.Artwork(R.drawable.flag_de),
            countryFlagSource(" DE "),
        )
    }

    @Test
    fun `a code the set lacks falls back to the emoji glyph`() {
        // Regional indicator Z twice: not a flag any font ships, so the glyph is
        // whatever the font renders for the pair, which is still the code.
        assertEquals(
            CountryFlagSource.Emoji("🇿🇿"),
            countryFlagSource("zz"),
        )
    }

    @Test
    fun `no code or a malformed code draws nothing`() {
        assertNull(countryFlagSource(null))
        assertNull(countryFlagSource(""))
        assertNull(countryFlagSource("1a"))
        assertNull(countryFlagSource("nld"))
    }
}
