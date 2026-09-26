package com.warrenbrowse.vpn.lib.ui.designsystem.networkstats

import androidx.compose.ui.geometry.Offset
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertTrue
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

    @Test
    fun `no values draw nothing`() {
        assertTrue(sparklinePoints(emptyList(), 100f, 20f, 2f).isEmpty())
    }

    @Test
    fun `a single value draws a flat line across the box`() {
        val points = sparklinePoints(listOf(5), 100f, 20f, 2f)

        assertEquals(listOf(Offset(2f, 2f), Offset(98f, 2f)), points)
    }

    @Test
    fun `values scale from zero at the bottom to the largest at the top`() {
        val points = sparklinePoints(listOf(0, 50, 100), 100f, 20f, 2f)

        assertEquals(listOf(Offset(2f, 18f), Offset(50f, 10f), Offset(98f, 2f)), points)
    }

    @Test
    fun `an all-zero series lies on the baseline`() {
        val points = sparklinePoints(listOf(0, 0), 100f, 20f, 2f)

        assertEquals(listOf(Offset(2f, 18f), Offset(98f, 18f)), points)
    }
}
