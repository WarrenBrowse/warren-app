package com.warrenbrowse.vpn.feature.home.impl.connect

import kotlin.test.assertEquals
import org.junit.jupiter.api.Test

/**
 * The vertical slide of the foreground pair (burrow + Bula) against the card.
 *
 * The three masters are painted registered on one canvas, so the foreground may
 * only ever slide DOWN: sliding it down covers more landscape, while lifting it
 * above its painted line uncovers the rows it was drawn to hide, the canal bed
 * at the bottom of the plain. A beta reporter saw exactly that when expanding
 * the connection details, which raises the card's top edge past Bula's feet.
 */
class SceneryForegroundShiftTest {

    // A 1080 px wide screen: the canvas ends 1616 px down and Bula's feet are
    // painted at 1262 px.
    private val feetY = 1080f * 1706f / 1140f * 1332f / 1706f
    private val gap = 16f

    @Test
    fun `the pair slides down to meet a card that sits low`() {
        // Connected, details collapsed: the card's top edge is below the feet.
        assertEquals(1584f - gap - feetY, foregroundShiftPx(1584f, gap, feetY), 0.01f)
    }

    @Test
    fun `the pair never lifts above the line it is painted on`() {
        // Details expanded: the card grows upwards, past the painted feet line.
        // Following it would uncover the landscape rows the burrow hides.
        assertEquals(0f, foregroundShiftPx(1100f, gap, feetY), 0.01f)
    }

    @Test
    fun `the pair stays put before the card has been laid out`() {
        assertEquals(0f, foregroundShiftPx(Float.NaN, gap, feetY), 0.01f)
    }
}
