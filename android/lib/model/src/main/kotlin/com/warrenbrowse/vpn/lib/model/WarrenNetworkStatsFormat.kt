package com.warrenbrowse.vpn.lib.model

import java.text.NumberFormat
import java.util.Locale
import kotlin.math.roundToLong
import kotlin.time.Duration
import kotlin.time.Duration.Companion.seconds

/** Locale-aware renderings of the snapshot's figures, shared by every surface. */
object NetworkStatsFormat {
    /** Bits per second in SI units: one decimal under ten, none from ten up. */
    fun bitsPerSecond(bps: Long, locale: Locale): String = si(bps, BIT_RATE_UNITS, locale)

    /** Bytes in SI units, same rounding as [bitsPerSecond]. */
    fun bytes(bytes: Long, locale: Locale): String = si(bytes, BYTE_UNITS, locale)

    /** `37%`, `37 %`: the sign and its spacing follow the locale. */
    fun percent(percent: Int, locale: Locale): String =
        NumberFormat.getPercentInstance(locale).format(percent / PERCENT)

    /** `1,234`, `40+`, `< 20`. */
    fun people(count: PeopleCount, locale: Locale): String {
        val number = NumberFormat.getIntegerInstance(locale)
        return when (count) {
            is PeopleCount.Exact -> number.format(count.count)
            is PeopleCount.AtLeast -> "${number.format(count.count)}+"
            // A no-break space, so a narrow row never wraps the sign away from its number.
            is PeopleCount.Below -> "<\u00A0${number.format(count.bound)}"
        }
    }

    private const val PERCENT = 100.0
    private const val STEP = 1000.0
    private const val DECIMAL_BELOW = 10.0
    private val BIT_RATE_UNITS = listOf("bit/s", "kbit/s", "Mbit/s", "Gbit/s", "Tbit/s")
    private val BYTE_UNITS = listOf("B", "kB", "MB", "GB", "TB", "PB")

    private fun si(value: Long, units: List<String>, locale: Locale): String {
        var scaled = value.coerceAtLeast(0).toDouble()
        var unit = 0
        while (unit < units.lastIndex && rounded(scaled) >= STEP) {
            scaled /= STEP
            unit += 1
        }
        // The base unit is never fractional.
        val decimals = if (unit > 0) decimals(scaled) else 0
        val number =
            NumberFormat.getNumberInstance(locale).apply {
                minimumFractionDigits = decimals
                maximumFractionDigits = decimals
            }
        return "${number.format(scaled)} ${units[unit]}"
    }

    // One decimal under ten, none from ten up; a value that rounds up to ten loses its
    // decimal too.
    private fun decimals(scaled: Double): Int =
        if ((scaled * DECIMAL_BELOW).roundToLong() / DECIMAL_BELOW < DECIMAL_BELOW) 1 else 0

    private fun rounded(scaled: Double): Double {
        val factor = if (decimals(scaled) == 1) DECIMAL_BELOW else 1.0
        return (scaled * factor).roundToLong() / factor
    }
}

/** How old a snapshot is, in the unit the "updated N ago" line uses. */
sealed interface SnapshotAge {
    data class Seconds(val value: Long) : SnapshotAge

    data class Minutes(val value: Long) : SnapshotAge

    data class Hours(val value: Long) : SnapshotAge
}

/** When a snapshot was taken, and when to ask for the next one. */
object NetworkStatsClock {
    /** A snapshot older than this many windows is kept on screen, greyed. */
    const val STALE_AFTER_WINDOWS = 3

    /** Seconds since the window the snapshot describes closed; never negative. */
    fun ageSecs(stats: WarrenNetworkStats, nowMillis: Long): Long =
        (nowMillis / MILLIS - stats.generatedAt).coerceAtLeast(0)

    fun isStale(stats: WarrenNetworkStats, nowMillis: Long): Boolean =
        ageSecs(stats, nowMillis) > STALE_AFTER_WINDOWS.toLong() * stats.windowSecs

    fun age(ageSecs: Long): SnapshotAge =
        when {
            ageSecs < MINUTE -> SnapshotAge.Seconds(ageSecs)
            ageSecs < HOUR -> SnapshotAge.Minutes(ageSecs / MINUTE)
            else -> SnapshotAge.Hours(ageSecs / HOUR)
        }

    /** One request per window, kept inside the window range the server accepts. */
    fun pollInterval(windowSecs: Int): Duration =
        windowSecs.coerceIn(MIN_WINDOW_SECS, MAX_WINDOW_SECS).seconds

    private const val MILLIS = 1000
    private const val MINUTE = 60
    private const val HOUR = 3600
    private const val MIN_WINDOW_SECS = 30
    private const val MAX_WINDOW_SECS = 3600
}
