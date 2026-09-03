package com.warrenbrowse.vpn.feature.language.impl

import java.util.Locale
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import org.junit.jupiter.api.Test

/**
 * The picker's offer used to come from the framework's `LocaleConfig`, which
 * exists only from API 33. It is now read off the app's own locale-config on
 * every API level, so this is where "which languages are offered, in which
 * order" is pinned.
 */
class SupportedLocalesTest {

    @Test
    fun orders_the_offered_languages_by_their_own_display_name() {
        val offered = supportedLocalesFromTags(listOf("sv", "de", "en-US", "fr"))

        // Sorted by endonym, not by tag: Deutsch, English, francais, svenska.
        assertEquals(listOf("de", "en", "fr", "sv"), offered.map { it.language })
    }

    @Test
    fun keeps_the_region_of_a_tag_that_carries_one() {
        val offered = supportedLocalesFromTags(listOf("en-US"))

        // The picker must hand back exactly the tag the locale-config declares:
        // a bare "en" resolves a different resource folder than "en-US".
        assertEquals("en-US", offered.single().toLanguageTag())
    }

    @Test
    fun drops_blank_and_duplicate_tags() {
        val offered = supportedLocalesFromTags(listOf("fr", "", "  ", "fr"))

        assertEquals(1, offered.size)
        assertEquals("fr", offered.single().toLanguageTag())
    }

    @Test
    fun offers_nothing_when_the_locale_config_is_empty() {
        // A missing or unreadable locale-config must leave the picker empty
        // rather than crash it: the system-default row still applies.
        assertTrue(supportedLocalesFromTags(emptyList()).isEmpty())
    }

    @Test
    fun the_selected_tag_is_matched_against_the_offer_by_language_tag() {
        val offered = supportedLocalesFromTags(listOf("en-US", "fr"))

        assertEquals(
            Locale.forLanguageTag("en-US").toLanguageTag(),
            offered.first { it.toLanguageTag() == "en-US" }.toLanguageTag(),
        )
    }
}
