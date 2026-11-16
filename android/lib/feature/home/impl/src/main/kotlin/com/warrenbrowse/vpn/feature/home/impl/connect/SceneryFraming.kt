package com.warrenbrowse.vpn.feature.home.impl.connect

/**
 * How the home hero frames the scenery, the formula every client computes the same way and that
 * `fixtures/client-rules/scenery_framing.json` pins. The three layers are registered on one canvas,
 * so they are always drawn with one transform; what changes is which rows of that canvas must be
 * fully visible between the floating header and the fade into the panel.
 */
internal object SceneryFraming {
    const val CANVAS_WIDTH = 1140f
    const val CANVAS_HEIGHT = 1706f

    /**
     * The canvas edge facing a headroom strip or a side margin fades over this many canvas rows
     * into the landscape's top fill, so the join does not show.
     */
    const val FEATHER_ROWS = 48f

    /** The mockup's proportions (Warren VPN Accueil, 360x640): a 300 high hero on a 360 width. */
    const val HERO_HEIGHT_RATIO = 300f / 360f

    /**
     * The hero's alpha from its top to its bottom, the mockup's own curve: `(fraction of the hero
     * height, alpha)`. The panel's status block rides up over the last rows, so the art has no edge.
     */
    val MASK_STOPS: List<Pair<Float, Float>> =
        listOf(
            0f to 1f,
            0.52f to 1f,
            0.64f to 0.82f,
            0.77f to 0.5f,
            0.89f to 0.2f,
            0.96f to 0.05f,
            1f to 0f,
        )

    /** Where [MASK_STOPS] reaches half opacity: the clear zone ends there. */
    const val CLEAR_BOTTOM_FRACTION = 0.77f

    /**
     * The cameras and Bula down to his hips while he is out; his legs and the burrow mouth fade into
     * the panel, as in the mockup.
     */
    val GROUND_SPAN = Span(top = 320f, bottom = 830f)

    /** Once Bula is in: the skyline down to the flag. */
    const val SKY_BOTTOM_ROW = 610f

    const val THUMB_MARGIN_ROWS = 40

    private const val SUBPIXEL = 0.5f

    /** A band of canvas rows. */
    data class Span(val top: Float, val bottom: Float)

    data class Frame(
        val scale: Float,
        val left: Float,
        val top: Float,
        val width: Float,
        val height: Float,
        /** The strip above a canvas that had to drop to clear the header. */
        val headroom: Float,
    ) {
        val feathersTop: Boolean
            get() = headroom > 0f

        // A width-fit canvas leaves float noise, not a margin, on either side.
        val feathersSides: Boolean
            get() = left > SUBPIXEL
    }

    fun frame(width: Float, height: Float, clearTop: Float, fade: Float, span: Span): Frame {
        val clearBottom = height - fade
        val scale =
            minOf(width / CANVAS_WIDTH, (clearBottom - clearTop) / (span.bottom - span.top))
        val yMin = clearTop - span.top * scale
        val yMax = clearBottom - span.bottom * scale
        val top = minOf(yMax, maxOf(yMin, 0f))
        return Frame(
            scale = scale,
            left = (width - CANVAS_WIDTH * scale) / 2f,
            top = top,
            width = CANVAS_WIDTH * scale,
            height = CANVAS_HEIGHT * scale,
            headroom = maxOf(0f, top),
        )
    }

    /**
     * The rows a phase must keep in view: the ground while Bula is out, the skyline of the
     * landscape the phase shows down to its flag once he is in.
     */
    fun span(phase: ConnectionPhase, exitCountry: String?): Span =
        if (SceneryTable.row(phase).skyFraming) {
            Span(landscapeOf(phase, exitCountry).skylineTopRow.toFloat(), SKY_BOTTOM_ROW)
        } else {
            GROUND_SPAN
        }

    /** The framing data of the landscape a phase shows. */
    fun landscapeOf(phase: ConnectionPhase, exitCountry: String?): SceneryLandscape =
        SceneryTable.landscapes.getValue(resolveScenery(phase, exitCountry).landscape)

    /**
     * The round location thumbnail is a full-width square of the landscape whose top sits just
     * above the skyline, so the landmark crowns it.
     */
    fun thumbnailTopRow(skylineTopRow: Int): Int =
        minOf(
            (CANVAS_HEIGHT - CANVAS_WIDTH).toInt(),
            maxOf(0, skylineTopRow - THUMB_MARGIN_ROWS),
        )

    /** The band under the clear zone, for a hero [height] tall. */
    fun fade(height: Float): Float = height * (1f - CLEAR_BOTTOM_FRACTION)

    /**
     * The hero at the mockup's ratio of its [width], drawn under the [statusBar] on a phone; the
     * panel takes the rest. Only a screen too short for both gives the hero less, down to the
     * [available] height the panel leaves, and the formula then narrows the canvas to fit.
     */
    fun heroHeight(width: Float, statusBar: Float, available: Float): Float =
        maxOf(0f, minOf(available, width * HERO_HEIGHT_RATIO + statusBar))
}
