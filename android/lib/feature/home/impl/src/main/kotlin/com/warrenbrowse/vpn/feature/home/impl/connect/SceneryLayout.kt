package com.warrenbrowse.vpn.feature.home.impl.connect

/**
 * Where each scenery layer lands on a screen, the one formula all three clients compute.
 *
 * The three masters are painted registered on one [CANVAS_WIDTH] x [CANVAS_HEIGHT] canvas and are
 * always drawn at full screen width with no side crop, so every edge element stays in frame (a
 * country flag sits at x 0.929 to 0.945 of the canvas width, the burrow mouth starts at 0.094). A
 * phone is far taller than the canvas, and the rows below it used to carry a mirrored, blurred copy
 * of the landscape: that band read as a hard full-width line and a smeared bottom quarter of the
 * screen. It is gone. Each layer is instead drawn in two parts, natural down to [GROUND_ROW] and
 * vertically scaled below it, which continues the painted ground itself.
 *
 * The shared fixture `fixtures/client-rules/scenery_layout.json` carries this formula, its
 * provenance and the cases every client replays.
 */
internal object SceneryLayout {

    /** The canvas every master is painted on. */
    const val CANVAS_WIDTH = 1140f
    const val CANVAS_HEIGHT = 1706f

    /**
     * Bula's painted body bottom (`bula.png`, last row with alpha >= 128). His cast shadow is a
     * separate flat-alpha tail reaching row 1326, and aiming the slide at that tail is what used to
     * place his feet 216 canvas rows above where the code said it wanted them.
     */
    const val FEET_ROW = 1124f

    /**
     * The first canvas row the burrow layer paints fully opaque across the whole width
     * (`terrier.png`, first row whose every column has alpha >= 250, contiguous to the last row).
     * Everything below it is one uniform watercolour meadow, which is why scaling only those rows
     * continues the ground with no seam and no repetition.
     */
    const val GROUND_ROW = 1301f

    /** Air kept between Bula's feet and the card's top edge. */
    const val GAP_DP = 16f

    /**
     * How far the whole canvas may pan up. It is the band the header already draws over, so a pan
     * can only ever crop rows the header was covering.
     */
    const val MAX_CANVAS_PAN_DP = 96f

    /**
     * The resolved placement, in screen pixels from the top of the backdrop.
     *
     * [canvasPan] is never positive and [foregroundShift] never negative, and exactly one of them
     * is ever non-zero: the foreground slides DOWN over the landscape to meet a card that sits low,
     * and when the feet would instead have to rise, the WHOLE canvas pans up by that much. Panning
     * keeps the three layers registered, so it cannot uncover the rows the burrow exists to hide,
     * which a bare lift did.
     */
    data class Placement(
        val scale: Float,
        val canvasHeight: Float,
        val canvasPan: Float,
        val foregroundShift: Float,
        val landscapeTop: Float,
        val foregroundTop: Float,
        val landscapeBottom: Float,
        val foregroundBottom: Float,
    ) {
        /** The canvas row the two-part draw splits at, in screen pixels from a layer's own top. */
        val groundOffset: Float
            get() = GROUND_ROW * scale
    }

    fun placement(
        screenWidth: Float,
        screenHeight: Float,
        cardTop: Float,
        density: Float,
    ): Placement {
        val scale = screenWidth / CANVAS_WIDTH
        val canvasHeight = CANVAS_HEIGHT * scale
        val feetY = FEET_ROW * scale
        val groundY = GROUND_ROW * scale
        // Before the card has been laid out there is nothing to track, so the canvas sits where it
        // is painted rather than guessing.
        val want = if (cardTop.isNaN()) 0f else cardTop - GAP_DP * density - feetY
        val panLimit = MAX_CANVAS_PAN_DP * density
        val panNeeded = minOf(panLimit, maxOf(0f, -want))
        // Spelled out rather than negated in place: negating a zero yields -0.0, which reads as a
        // pan in a log and is not equal to 0 under Float.equals.
        val canvasPan = if (panNeeded == 0f) 0f else -panNeeded
        // The slide is capped so the burrow layer's opaque rows always still overlap the landscape
        // above them. Past that cap a window would open between the landscape's bottom edge and
        // the row the burrow turns opaque at, and the only ways to close it are to stretch the
        // landscape, whose lower rows are a canal and its banks rather than a uniform wash (the
        // scale step read as a line across the water), or to repeat its last row, which is pale
        // paper at the edges and read as a white strip. Capping costs Bula a little height above
        // the card on the tallest screens and costs the art nothing.
        val maxShift = canvasHeight - groundY
        val foregroundShift = minOf(maxShift, maxOf(0f, want - canvasPan))
        val foregroundTop = canvasPan + foregroundShift
        return Placement(
            scale = scale,
            canvasHeight = canvasHeight,
            canvasPan = canvasPan,
            foregroundShift = foregroundShift,
            landscapeTop = canvasPan,
            foregroundTop = foregroundTop,
            // The landscape is never stretched: it is drawn whole, and everything below it is the
            // burrow layer's own opaque ground.
            landscapeBottom = canvasPan + canvasHeight,
            foregroundBottom = maxOf(foregroundTop + canvasHeight, screenHeight),
        )
    }
}
