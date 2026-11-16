package com.warrenbrowse.vpn.feature.home.impl.connect

/**
 * How the home fills a screen of any size. The mockup is a fixed 380 x 620 window; a phone or a
 * tablet is not, so the hero is the part that gives: it absorbs the free height of a phone between
 * the mockup's ratio of its width and [SceneryFraming.HERO_MAX_HEIGHT_RATIO], the panel keeps its
 * natural height right under it, and only what is left beyond the hero's max becomes the gap above
 * the footer. A screen too short for the mockup's ratio gives the hero less, down to the compact
 * hero, and below that the content scrolls with the footer still pinned. Open details shrink the
 * hero toward the compact hero only as far as the card needs, so a tall phone opens no hole above
 * the footer while a phone with no room to spare reaches the compact ratio. A wide screen keeps its
 * header, panel and footer in a centred column of at most [COLUMN_MAX_WIDTH] while the hero spans
 * the whole width: the column's rule is the hero's floor, so a landscape tablet keeps its controls
 * on screen, and the free height is taken up to the max of the hero's own width, so a portrait
 * tablet opens no hole above its footer either. A phone in landscape ([splits]) puts the hero, the
 * left half at full height, beside the panel instead.
 */
internal object HomeLayout {
    const val COLUMN_MAX_WIDTH = 480f

    /**
     * A screen wider than tall and shorter than this (a phone in landscape) cannot hold the hero
     * over the panel: it puts them side by side instead.
     */
    const val SPLIT_BELOW_HEIGHT = 560f

    /** Whether a [width] x [height] screen lays the hero and the panel side by side. */
    fun splits(width: Float, height: Float): Boolean =
        width > height && height < SPLIT_BELOW_HEIGHT

    data class Plan(
        /** The width of the header, panel and footer column. */
        val column: Float,
        /** The hero's height on this frame, partway through a compact resize. */
        val hero: Float,
        /** The height the hero is heading to, which the art is framed for. */
        val frameHero: Float,
        /** The hero's height with the details closed, whose scale a compact hero keeps. */
        val fullHero: Float,
        /** The height of the hero and the panel together, at least the viewport. */
        val content: Float,
    ) {
        fun scrolls(viewport: Float): Boolean = content > viewport + SUBPIXEL
    }

    /**
     * The plan for a screen [width] wide whose [viewport] is its height above the footer, with the
     * [statusBar] over the hero, the panel riding [overlap] up over the hero's fade, a [panel] of
     * its natural height without the details card, the [card] at its current height and
     * [cardFull] once open. The hero is [progress] of the way to its height with the card open,
     * which [compact] says it is heading to.
     */
    @Suppress("LongParameterList")
    fun plan(
        width: Float,
        viewport: Float,
        statusBar: Float,
        overlap: Float,
        panel: Float,
        card: Float,
        cardFull: Float,
        compact: Boolean,
        progress: Float,
    ): Plan {
        val column = minOf(width, COLUMN_MAX_WIDTH)
        // On a phone the column is the width, and this is SceneryFraming.heroHeight.
        val natural =
            (viewport - panel + overlap).coerceIn(
                SceneryFraming.compactHeroHeight(column, statusBar),
                width * SceneryFraming.HERO_MAX_HEIGHT_RATIO + statusBar,
            )
        val open =
            (viewport - panel - cardFull + overlap).coerceIn(
                SceneryFraming.compactHeroHeight(column, statusBar),
                natural,
            )
        val hero = natural + (open - natural) * progress
        return Plan(
            column = column,
            hero = hero,
            frameHero = if (compact) open else natural,
            fullHero = natural,
            content = maxOf(viewport, hero - overlap + panel + card),
        )
    }

    private const val SUBPIXEL = 0.5f
}
