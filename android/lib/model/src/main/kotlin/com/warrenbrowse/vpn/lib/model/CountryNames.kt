package com.warrenbrowse.vpn.lib.model

import java.util.Locale

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
 */
fun countryDisplayName(code: String, locale: Locale = Locale.getDefault()): String {
    val trimmed = code.trim()
    if (trimmed.length != 2 || !trimmed.all(Char::isLetter)) return trimmed
    val display =
        Locale.Builder().setRegion(trimmed.uppercase(Locale.ROOT)).build().getDisplayCountry(locale)
    return display.ifBlank { trimmed }
}
