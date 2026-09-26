package com.warrenbrowse.vpn.lib.model

import java.util.Locale
import kotlin.time.Duration.Companion.hours
import kotlin.time.Duration.Companion.seconds
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

class WarrenNetworkStatsFormatTest {

    private val en = Locale.ENGLISH
    private val fr = Locale.FRENCH

    @Test
    fun `bit rates use SI units with one decimal under ten`() {
        assertEquals("4.2 Mbit/s", NetworkStatsFormat.bitsPerSecond(4_200_000, en))
    }

    @Test
    fun `bit rates drop the decimal from ten up`() {
        assertEquals("300 Mbit/s", NetworkStatsFormat.bitsPerSecond(300_000_000, en))
    }

    @Test
    fun `a value that rounds to a thousand moves to the next unit`() {
        assertEquals("1.0 Mbit/s", NetworkStatsFormat.bitsPerSecond(999_700, en))
    }

    @Test
    fun `a decimal that rounds to ten is not shown`() {
        assertEquals("10 Mbit/s", NetworkStatsFormat.bitsPerSecond(9_960_000, en))
    }

    @Test
    fun `plain bits carry no decimal`() {
        assertEquals("950 bit/s", NetworkStatsFormat.bitsPerSecond(950, en))
        assertEquals("0 bit/s", NetworkStatsFormat.bitsPerSecond(0, en))
    }

    @Test
    fun `the decimal separator follows the locale`() {
        assertEquals("1,5 Gbit/s", NetworkStatsFormat.bitsPerSecond(1_500_000_000, fr))
    }

    @Test
    fun `bytes use SI units`() {
        assertEquals("9.0 TB", NetworkStatsFormat.bytes(9_000_000_000_000, en))
        assertEquals("512 B", NetworkStatsFormat.bytes(512, en))
        assertEquals("42 MB", NetworkStatsFormat.bytes(42_000_000, en))
    }

    @Test
    fun `percentages follow the locale`() {
        assertEquals("37%", NetworkStatsFormat.percent(37, en))
        // French puts a no-break space before the sign; which one depends on the
        // CLDR data of the runtime.
        assertTrue(Regex("^37\\s?[\u00A0\u202F]?%$").matches(NetworkStatsFormat.percent(37, fr)))
    }

    @Test
    fun `people counts render exact, as a floor, or as a bound`() {
        assertEquals("1,234", NetworkStatsFormat.people(PeopleCount.Exact(1234), en))
        assertEquals("40+", NetworkStatsFormat.people(PeopleCount.AtLeast(40), en))
        assertEquals("<\u00A020", NetworkStatsFormat.people(PeopleCount.Below(20), en))
    }

    private val stats =
        WarrenNetworkStatsParser.parse(
                requireNotNull(javaClass.getResource("/network-stats-v1.json")).readText()
            )!!
            .copy(generatedAt = 1_000, windowSecs = 60)

    @Test
    fun `age runs from the moment the window closed`() {
        assertEquals(42, NetworkStatsClock.ageSecs(stats, 1_042_500))
    }

    @Test
    fun `age is never negative when the local clock runs behind`() {
        assertEquals(0, NetworkStatsClock.ageSecs(stats, 900_000))
    }

    @Test
    fun `a snapshot goes stale once older than three windows`() {
        assertFalse(NetworkStatsClock.isStale(stats, 1_180_000))
        assertTrue(NetworkStatsClock.isStale(stats, 1_181_000))
    }

    @Test
    fun `the age is told in seconds, then minutes, then hours`() {
        assertEquals(SnapshotAge.Seconds(59), NetworkStatsClock.age(59))
        assertEquals(SnapshotAge.Minutes(1), NetworkStatsClock.age(60))
        assertEquals(SnapshotAge.Minutes(59), NetworkStatsClock.age(3_599))
        assertEquals(SnapshotAge.Hours(1), NetworkStatsClock.age(3_600))
        assertEquals(SnapshotAge.Hours(2), NetworkStatsClock.age(7_300))
    }

    @Test
    fun `polls once per window, inside the range the server accepts`() {
        assertEquals(60.seconds, NetworkStatsClock.pollInterval(60))
        assertEquals(30.seconds, NetworkStatsClock.pollInterval(1))
        assertEquals(1.hours, NetworkStatsClock.pollInterval(100_000))
    }
}
