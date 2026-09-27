package com.warrenbrowse.vpn.lib.model

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Test

class ExitLoadBadgeTest {

    private val stats: WarrenNetworkStats =
        WarrenNetworkStatsParser.parse(
            requireNotNull(javaClass.getResource("/network-stats-v1.json")).readText()
        )!!

    private val live = stats.exits[0]
    private val quiet = stats.exits[1]

    @Test
    fun `a live exit shows its percentage, a people floor and its download rate`() {
        assertEquals(
            ExitLoadBadge.Shown(
                level = LoadLevel.LOW,
                arcFraction = 0.37f,
                percent = 37,
                people = PeopleCount.AtLeast(40),
                downloadBps = 300_000_000,
            ),
            stats.loadBadgeOf(live),
        )
    }

    @Test
    fun `a quiet exit shows its band as a fixed arc, the threshold, and no figure`() {
        val badge = stats.loadBadgeOf(quiet) as ExitLoadBadge.Shown

        assertEquals(LoadLevel.MODERATE, badge.level)
        assertEquals(0.6f, badge.arcFraction)
        assertNull(badge.percent)
        assertEquals(PeopleCount.Below(20), badge.people)
        assertNull(badge.downloadBps)
    }

    @Test
    fun `a quiet exit's arc carries its band, so the level never rests on colour alone`() {
        fun arcOf(level: LoadLevel) =
            (stats.loadBadgeOf(quiet.copy(loadLevel = level)) as ExitLoadBadge.Shown).arcFraction

        assertEquals(0.25f, arcOf(LoadLevel.LOW))
        assertEquals(0.6f, arcOf(LoadLevel.MODERATE))
        assertEquals(0.85f, arcOf(LoadLevel.HIGH))
        assertEquals(1f, arcOf(LoadLevel.SATURATED))
        assertEquals(0f, arcOf(LoadLevel.UNKNOWN))
    }

    @Test
    fun `a quiet exit hides the figures it happens to carry`() {
        val badge =
            stats.loadBadgeOf(quiet.copy(loadPercent = 55, downloadBps = 9_000, connected = 15))
                as ExitLoadBadge.Shown

        assertNull(badge.percent)
        assertNull(badge.downloadBps)
        assertEquals(PeopleCount.Below(20), badge.people)
    }

    @Test
    fun `a live exit without a percentage falls back to its band's arc`() {
        val badge = stats.loadBadgeOf(live.copy(loadPercent = null)) as ExitLoadBadge.Shown

        assertNull(badge.percent)
        assertEquals(0.25f, badge.arcFraction)
        assertEquals(300_000_000L, badge.downloadBps)
    }

    @Test
    fun `a live arc is clamped to a full turn`() {
        val over = stats.loadBadgeOf(live.copy(loadPercent = 130)) as ExitLoadBadge.Shown
        val under = stats.loadBadgeOf(live.copy(loadPercent = -4)) as ExitLoadBadge.Shown

        assertEquals(1f, over.arcFraction)
        assertEquals(0f, under.arcFraction)
    }

    @Test
    fun `an offline exit shows nothing but offline, even when it claims to be live`() {
        assertEquals(ExitLoadBadge.Offline, stats.loadBadgeOf(live.copy(online = false)))
        assertEquals(ExitLoadBadge.Offline, stats.loadBadgeOf(quiet.copy(online = false)))
    }
}
