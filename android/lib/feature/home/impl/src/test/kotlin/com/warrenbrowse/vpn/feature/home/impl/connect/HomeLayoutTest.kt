package com.warrenbrowse.vpn.feature.home.impl.connect

import kotlin.test.assertEquals
import kotlin.test.assertTrue
import org.junit.jupiter.api.Test

/**
 * The home's hero height for a given screen: it absorbs the free height of a phone up to its max,
 * gives way down to the compact hero on a short screen, below which the content scrolls, and takes
 * its proportions from a centred column of at most 480 on a wide screen.
 */
class HomeLayoutTest {

    private fun plan(
        width: Float,
        viewport: Float,
        statusBar: Float = STATUS_BAR,
        panel: Float = PANEL,
        card: Float = 0f,
        cardFull: Float = card,
        compact: Boolean = false,
        progress: Float = if (compact) 1f else 0f,
    ) =
        HomeLayout.plan(
            width = width,
            viewport = viewport,
            statusBar = statusBar,
            overlap = OVERLAP,
            panel = panel,
            card = card,
            cardFull = cardFull,
            compact = compact,
            progress = progress,
        )

    @Test
    fun `a tall phone gives the hero its max and the rest to the gap above the footer`() {
        val plan = plan(width = 412f, viewport = 860f)
        assertEquals(412f * SceneryFraming.HERO_MAX_HEIGHT_RATIO + STATUS_BAR, plan.hero)
        assertEquals(860f, plan.content)
        assertTrue(!plan.scrolls(860f))
    }

    @Test
    fun `a phone with some free height gives all of it to the hero`() {
        val viewport = 412f * SceneryFraming.HERO_HEIGHT_RATIO + STATUS_BAR + PANEL - OVERLAP + 50f
        val plan = plan(width = 412f, viewport = viewport)
        assertEquals(viewport - PANEL + OVERLAP, plan.hero, TOLERANCE)
        assertEquals(viewport, plan.content, TOLERANCE)
    }

    @Test
    fun `a short screen shrinks the hero to the compact one, then scrolls`() {
        val plan = plan(width = 640f, viewport = 300f, statusBar = 0f)
        val column = HomeLayout.COLUMN_MAX_WIDTH
        assertEquals(column * SceneryFraming.COMPACT_HEIGHT_RATIO, plan.hero, TOLERANCE)
        assertEquals(plan.hero - OVERLAP + PANEL, plan.content, TOLERANCE)
        assertTrue(plan.scrolls(300f))
    }

    @Test
    fun `a wide screen keeps a column of at most 480, whose rule is the hero's floor`() {
        val plan = plan(width = 1280f, viewport = 736f)
        assertEquals(HomeLayout.COLUMN_MAX_WIDTH, plan.column)
        assertEquals(736f - PANEL + OVERLAP, plan.hero, TOLERANCE)
        assertEquals(360f, plan(width = 360f, viewport = 600f).column)
        val short = plan(width = 1280f, viewport = 300f)
        assertEquals(
            HomeLayout.COLUMN_MAX_WIDTH * SceneryFraming.COMPACT_HEIGHT_RATIO + STATUS_BAR,
            short.hero,
            TOLERANCE,
        )
    }

    @Test
    fun `a tall tablet's hero takes the free height up to its own width's max, leaving no hole`() {
        val plan = plan(width = 800f, viewport = 1216f)
        assertEquals(1216f - PANEL + OVERLAP, plan.hero, TOLERANCE)
        assertEquals(1216f, plan.hero - OVERLAP + PANEL, TOLERANCE)
        val taller = plan(width = 800f, viewport = 3000f)
        assertEquals(
            800f * SceneryFraming.HERO_MAX_HEIGHT_RATIO + STATUS_BAR,
            taller.hero,
            TOLERANCE,
        )
    }

    @Test
    fun `open details shrink the hero to the compact ratio on a phone with no room to spare`() {
        val compact = 360f * SceneryFraming.COMPACT_HEIGHT_RATIO + STATUS_BAR
        val natural = 360f * SceneryFraming.HERO_HEIGHT_RATIO + STATUS_BAR
        val viewport = natural - OVERLAP + PANEL
        val open = plan(width = 360f, viewport = viewport, card = 150f, compact = true)
        assertEquals(compact, open.hero, TOLERANCE)
        assertEquals(compact, open.frameHero, TOLERANCE)
        assertEquals(natural, open.fullHero, TOLERANCE)
        // Halfway through the resize the hero is halfway, the art already framed for the end.
        val half =
            plan(
                width = 360f,
                viewport = viewport,
                card = 75f,
                cardFull = 150f,
                compact = true,
                progress = 0.5f,
            )
        assertEquals((natural + compact) / 2f, half.hero, TOLERANCE)
        assertEquals(compact, half.frameHero, TOLERANCE)
        val closing =
            plan(width = 360f, viewport = viewport, card = 75f, cardFull = 150f, progress = 0.5f)
        assertEquals(natural, closing.frameHero, TOLERANCE)
    }

    @Test
    fun `on a tall phone the hero gives the card only the room the gap cannot`() {
        val natural = 412f * SceneryFraming.HERO_MAX_HEIGHT_RATIO + STATUS_BAR
        val gap = 860f - (natural - OVERLAP + PANEL)
        val open = plan(width = 412f, viewport = 860f, card = 150f, compact = true)
        assertEquals(natural - (150f - gap), open.hero, TOLERANCE)
        assertEquals(860f, open.content, TOLERANCE)
        val roomy = plan(width = 412f, viewport = 2000f, card = 150f, compact = true)
        assertEquals(natural, roomy.hero, TOLERANCE)
    }

    @Test
    fun `the card pushes the content and scrolls it when the screen is full`() {
        val plan = plan(width = 360f, viewport = 560f, card = 200f, compact = true)
        assertEquals(plan.hero - OVERLAP + PANEL + 200f, plan.content, TOLERANCE)
        assertTrue(plan.scrolls(560f))
    }

    private companion object {
        const val STATUS_BAR = 24f
        const val OVERLAP = 18f
        const val PANEL = 300f
        const val TOLERANCE = 0.01f
    }
}
