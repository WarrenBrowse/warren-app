package com.warrenbrowse.vpn.lib.model

import java.util.Locale
import java.util.concurrent.ConcurrentHashMap

/**
 * Localized display name for an ISO 3166-1 alpha-2 country code, e.g. "de" ->
 * "Germany" (or "Allemagne" under a French UI). Warren's relay directory carries
 * only the raw code, while the desktop shows the human country name; this maps
 * the wire code to the name for display without disturbing the code, which stays
 * the key for scenery art and flag-emoji lookups.
 *
 * Anything that is not a two-letter code (an already-humanized name, a blank) is
 * returned unchanged, and an unknown region falls back to the raw code, so the
 * caller always has something to render.
 *
 * The lookup is memoised per code and locale ([CountryNames]): the picker
 * calls it inside its sort comparator and once per relay per keystroke, on the
 * main thread, and ICU answers from locale data every time.
 */
fun countryDisplayName(code: String, locale: Locale = Locale.getDefault()): String {
    val trimmed = code.trim()
    if (trimmed.length != 2 || !trimmed.all(Char::isLetter)) return trimmed
    return CountryNames.displayName(trimmed.uppercase(Locale.ROOT), locale)
}

/** The memo behind [countryDisplayName]; bounded by the ISO country list per locale. */
object CountryNames {
    private val names = ConcurrentHashMap<String, String>()

    internal fun displayName(region: String, locale: Locale): String =
        names.getOrPut("$region|${locale.toLanguageTag()}") {
            Locale.Builder().setRegion(region).build().getDisplayCountry(locale).ifBlank { region }
        }

    fun size(): Int = names.size

    fun clear() = names.clear()
}
