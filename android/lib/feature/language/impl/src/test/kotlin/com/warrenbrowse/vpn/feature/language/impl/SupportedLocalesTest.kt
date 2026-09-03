package com.warrenbrowse.vpn.feature.language.impl

import java.io.File
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
    fun reads_every_tag_the_shipped_locale_config_declares() {
        // The whole offer rests on this reader, and a wrong namespace or tag
        // name empties the picker on every device with nothing to see.
        val tags = readLocaleConfigTags(StaxPullParser(shippedLocaleConfig()))

        assertEquals(24, tags.size, "the shipped locale-config lost languages")
        assertEquals("en-US", tags.first(), "the region-qualified tag must survive the read")
        assertTrue(tags.containsAll(listOf("ar", "fr", "my", "th", "zh-CN", "zh-TW")))
        assertEquals(tags.size, tags.distinct().size)
    }

    @Test
    fun the_shipped_locale_config_reaches_the_picker_as_offered_languages() {
        val offered = supportedLocalesFromTags(readLocaleConfigTags(StaxPullParser(shippedLocaleConfig())))

        assertEquals(24, offered.size)
        assertTrue(
            offered.any { it.toLanguageTag() == "en-US" },
            "a bare \"en\" resolves a different resource folder than \"en-US\"",
        )
    }

    @Test
    fun ignores_a_name_declared_in_another_namespace() {
        // `getAttributeValue` is asked for the android namespace by name: a tag
        // carrying a bare `name` is not a declaration this reader accepts.
        val tags =
            readLocaleConfigTags(
                StaxPullParser(
                    """<locale-config><locale name="fr"/></locale-config>"""
                )
            )

        assertTrue(tags.isEmpty())
    }

    @Test
    fun malformed_xml_leaves_the_picker_empty_instead_of_throwing() {
        // The picker still shows the system-default row, which applies a
        // language; a throw here would take the whole settings screen down.
        val tags =
            readLocaleConfigTags(
                StaxPullParser(
                    """<locale-config xmlns:android="$ANDROID_NS"><locale android:name="fr">"""
                )
            )

        assertTrue(tags.isEmpty())
    }

    @Test
    fun a_tag_with_no_name_at_all_is_skipped() {
        val tags =
            readLocaleConfigTags(
                StaxPullParser(
                    """<locale-config xmlns:android="$ANDROID_NS"><locale/>""" +
                        """<locale android:name="fr"/></locale-config>"""
                )
            )

        assertEquals(listOf("fr"), tags)
    }

    /** The shipped `locales_config.xml`, read from the resource module. */
    private fun shippedLocaleConfig(): String {
        var dir: File? = File("").absoluteFile
        while (dir != null) {
            val candidate = File(dir, "lib/ui/resource/src/main/res/xml/locales_config.xml")
            if (candidate.isFile) {
                return candidate.readText()
            }
            dir = dir.parentFile
        }
        error("locales_config.xml not found from ${File("").absolutePath}")
    }
}

private const val ANDROID_NS = "http://schemas.android.com/apk/res/android"
