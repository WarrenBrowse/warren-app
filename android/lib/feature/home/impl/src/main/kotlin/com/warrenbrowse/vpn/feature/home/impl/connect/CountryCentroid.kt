package com.warrenbrowse.vpn.feature.home.impl.connect

/**
 * ISO 3166-1 alpha-2 country code -> approximate geographic centroid
 * (latitude, longitude in degrees).
 *
 * Warren's signed `/v1/exits` relay list carries only a country code and city
 * name, no coordinates, so the home-map marker has no lat/long to plot. This
 * table supplies a per-country centroid, mirroring the desktop which plots the
 * selected relay's city centroid (and, while disconnected, falls back to the
 * selected relay because Warren has no device-GeoIP service). Lookup is
 * case-insensitive; an unknown code returns null (no marker).
 */
internal object CountryCentroid {
    fun of(code: String?): Pair<Double, Double>? =
        code?.lowercase()?.let(CENTROIDS::get)

    private val CENTROIDS: Map<String, Pair<Double, Double>> = mapOf(
        "de" to (51.1 to 10.4),
        "nl" to (52.1 to 5.3),
        "sg" to (1.35 to 103.8),
        "us" to (39.8 to -98.6),
        "gb" to (54.0 to -2.0),
        "uk" to (54.0 to -2.0),
        "fr" to (46.6 to 2.2),
        "se" to (62.2 to 14.6),
        "ch" to (46.8 to 8.2),
        "ca" to (56.1 to -106.3),
        "au" to (-25.3 to 133.8),
        "jp" to (36.2 to 138.3),
        "fi" to (64.0 to 26.0),
        "no" to (64.5 to 17.9),
        "dk" to (56.3 to 9.5),
        "es" to (40.2 to -3.7),
        "it" to (41.9 to 12.6),
        "at" to (47.5 to 14.6),
        "be" to (50.5 to 4.5),
        "pl" to (51.9 to 19.1),
        "ro" to (45.9 to 24.9),
        "cz" to (49.8 to 15.5),
        "ie" to (53.4 to -8.0),
        "pt" to (39.4 to -8.2),
        "gr" to (39.1 to 21.8),
        "bg" to (42.7 to 25.5),
        "hu" to (47.2 to 19.5),
        "ua" to (48.4 to 31.2),
        "in" to (22.0 to 79.0),
        "hk" to (22.3 to 114.2),
        "kr" to (35.9 to 127.8),
        "br" to (-14.2 to -51.9),
        "za" to (-30.6 to 22.9),
        "mx" to (23.6 to -102.5),
        "nz" to (-40.9 to 174.9),
        "ae" to (23.4 to 53.8),
        "il" to (31.0 to 34.8),
        "tr" to (38.9 to 35.2),
        "sk" to (48.7 to 19.7),
        "rs" to (44.0 to 21.0),
        "ee" to (58.6 to 25.0),
        "lv" to (56.9 to 24.6),
        "lt" to (55.2 to 23.9),
        "lu" to (49.8 to 6.1),
        "si" to (46.1 to 14.8),
        "hr" to (45.1 to 15.2),
    )
}
