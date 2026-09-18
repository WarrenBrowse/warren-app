package com.warrenbrowse.vpn.feature.home.impl.connect

import kotlin.test.assertEquals
import kotlin.test.assertTrue
import org.junit.jupiter.api.Test

/**
 * How the scene meets the connection card.
 *
 * The three masters are painted registered on one canvas, so the foreground may only ever slide
 * DOWN: sliding it down covers more landscape, while lifting it above its painted line uncovers the
 * rows it was drawn to hide, the canal bed at the bottom of the plain. A beta reporter saw exactly
 * that when expanding the connection details, which raises the card's top edge past Bula's feet.
 *
 * When the feet would have to rise anyway (a short screen, or the details expanded), the WHOLE
 * canvas pans up instead. That keeps the three layers registered, so it cannot uncover anything,
 * and it is capped at the band the header already draws over.
 */
class SceneryForegroundShiftTest {

    private val width = 1080f
    private val height = 2400f
    private val density = 2.625f
    private val scale = width / SceneryLayout.CANVAS_WIDTH
    private val feetY = SceneryLayout.FEET_ROW * scale
    private val gap = SceneryLayout.GAP_DP * density

    private fun placement(cardTop: Float) = SceneryLayout.placement(width, height, cardTop, density)

    @Test
    fun `the pair slides down to meet a card that sits low`() {
        val cardTop = 1400f
        val got = placement(cardTop)
        assertEquals(cardTop - gap - feetY, got.foregroundShift, 0.01f)
        assertEquals(0f, got.canvasPan)
        assertEquals(cardTop - gap, got.foregroundTop + feetY, 0.01f)
    }

    @Test
    fun `the slide stops where the opaque ground would leave the landscape behind it`() {
        // A card low enough to ask for more slide than the layers can give: past this the
        // landscape's bottom edge would show under the burrow's slope.
        val got = placement(2000f)
        val groundY = SceneryLayout.GROUND_ROW * scale
        assertEquals(got.canvasHeight - groundY, got.foregroundShift, 0.01f)
        assertTrue(
            got.foregroundTop + groundY <= got.landscapeBottom + 0.01f,
            "the opaque ground left the landscape behind it",
        )
    }

    @Test
    fun `the pair never lifts above the line it is painted on`() {
        // Details expanded: the card grows upwards, past the painted feet line.
        assertEquals(0f, placement(1100f).foregroundShift, 0.01f)
    }

    @Test
    fun `a card above the feet pans the whole canvas instead of lifting the pair`() {
        val cardTop = 1100f
        val got = placement(cardTop)
        // The pan moves every layer by the same amount, so nothing is uncovered.
        assertEquals(got.canvasPan, got.landscapeTop, 0.01f)
        assertEquals(got.canvasPan, got.foregroundTop, 0.01f)
        // And it buys exactly the room the card asked for.
        assertEquals(cardTop - gap, got.foregroundTop + feetY, 0.01f)
    }

    @Test
    fun `the pan stops where the flag would leave the top of the screen`() {
        // A card so high that following it would crop painted content. The pan stops at the sky
        // above the flag, which is the last row that may go: the flag itself must stay in frame.
        // On this screen that is the wider of the two caps, the other being the header band.
        val got = placement(0f)
        assertEquals(-SceneryLayout.FLAG_TOP_ROW * got.scale, got.canvasPan, 0.01f)
        assertEquals(0f, got.landscapeTop + SceneryLayout.FLAG_TOP_ROW * got.scale, 0.01f)
        assertEquals(0f, got.foregroundShift, 0.01f)
    }

    @Test
    fun `a card leaving no room drops the pair rather than burying it`() {
        // Nothing can clear a card whose top edge is the top of the screen, so the country art
        // stands alone instead of carrying a rabbit sunk behind the card.
        assertTrue(!placement(0f).showsForeground)
        assertTrue(placement(1400f).showsForeground)
    }

    @Test
    fun `the scene stays put before the card has been laid out`() {
        val got = placement(Float.NaN)
        assertEquals(0f, got.canvasPan)
        assertEquals(0f, got.foregroundShift)
    }

    @Test
    fun `the foreground always reaches the bottom of the screen`() {
        listOf(1584f, 1100f, 0f, Float.NaN).forEach { cardTop ->
            assertTrue(
                placement(cardTop).foregroundBottom >= height,
                "cardTop=$cardTop left a band below the foreground",
            )
        }
    }
}
