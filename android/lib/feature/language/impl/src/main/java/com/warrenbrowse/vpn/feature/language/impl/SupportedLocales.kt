package com.warrenbrowse.vpn.feature.language.impl

import android.content.res.Resources
import java.io.IOException
import java.util.Locale
import org.xmlpull.v1.XmlPullParser
import org.xmlpull.v1.XmlPullParserException

private const val ANDROID_RES_NS = "http://schemas.android.com/apk/res/android"
private const val LOCALE_TAG = "locale"

/**
 * The languages the picker offers, from the tags the app's locale-config
 * declares.
 *
 * The framework's own `LocaleConfig` reader exists only from API 33, and the
 * picker has to offer the same list on every device, so the config file the
 * manifest already points at is parsed directly. Ordering is by endonym: a
 * reader looking for their language scans for the word they write it with.
 */
internal fun supportedLocalesFromTags(tags: List<String>): List<Locale> =
    tags
        .map { it.trim() }
        .filter { it.isNotEmpty() }
        .distinct()
        .map { Locale.forLanguageTag(it) }
        .sortedBy { it.getDisplayName(it).lowercase() }

/**
 * Reads the locale tags out of the `<locale-config>` resource. A malformed or
 * unreadable config yields an empty offer rather than throwing: the picker
 * then shows the system-default row alone, which still applies a language.
 */
internal fun readLocaleConfigTags(resources: Resources, localeConfigResId: Int): List<String> {
    val parser =
        try {
            resources.getXml(localeConfigResId)
        } catch (e: Resources.NotFoundException) {
            return emptyList()
        }
    return try {
        buildList {
            var event = parser.eventType
            while (event != XmlPullParser.END_DOCUMENT) {
                if (event == XmlPullParser.START_TAG && parser.name == LOCALE_TAG) {
                    parser.getAttributeValue(ANDROID_RES_NS, "name")?.let(::add)
                }
                event = parser.next()
            }
        }
    } catch (e: XmlPullParserException) {
        emptyList()
    } catch (e: IOException) {
        emptyList()
    } finally {
        parser.close()
    }
}
