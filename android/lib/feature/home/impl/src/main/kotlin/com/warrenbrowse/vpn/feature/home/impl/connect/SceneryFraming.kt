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

    /** The cameras and the whole of Bula; the burrow mouth lies inside. */
    val GROUND_SPAN = Span(top = 320f, bottom = 1131f)

    /** Once Bula is in: the skyline down to the burrow mouth he hides in. */
    const val SKY_BOTTOM_ROW = 930f

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
     * landscape the phase shows down to the burrow once he is in.
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

    /**
     * The hero is never taller than the width-fit canvas: with the canvas top at or above the hero
     * top it then always reaches the fade, and with headroom it always reaches the hero bottom.
     */
    fun heroHeight(available: Float, width: Float): Float =
        maxOf(0f, minOf(available, CANVAS_HEIGHT * width / CANVAS_WIDTH))
}
