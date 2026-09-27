package com.warrenbrowse.vpn.lib.ui.designsystem.networkstats

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Test

class NetworkStatsGeometryTest {

    @Test
    fun `the arc covers the percentage of a full turn`() {
        assertEquals(0f, loadRingSweepDegrees(0f))
        assertEquals(133.2f, loadRingSweepDegrees(37f), 0.001f)
        assertEquals(360f, loadRingSweepDegrees(100f))
    }

    @Test
    fun `the arc never overruns a full turn nor runs backwards`() {
        assertEquals(360f, loadRingSweepDegrees(150f))
        assertEquals(0f, loadRingSweepDegrees(-5f))
    }
}
