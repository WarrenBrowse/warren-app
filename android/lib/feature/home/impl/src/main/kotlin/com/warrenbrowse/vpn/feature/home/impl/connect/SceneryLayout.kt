package com.warrenbrowse.vpn.feature.home.impl.connect

/**
 * Where each scenery layer lands on a screen, the one formula all three clients compute.
 *
 * The three masters are painted registered on one [CANVAS_WIDTH] x [CANVAS_HEIGHT] canvas and are
 * never side cropped, so every edge element stays in frame (a country flag sits at x 0.929 to 0.945
 * of the canvas width, the burrow mouth starts at 0.094). They fill the screen width where the
 * screen is tall enough to carry the flag down to Bula's feet at that scale, and shrink to fit that
 * span otherwise, which is what any landscape geometry needs; see [FLAG_TOP_ROW]. A
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

    /**
     * The last canvas row of the meadow before the wash runs out into bare paper: the band's row
     * mean climbs 34 levels of 255 over rows 1681 to 1705. The band is scaled so this row lands on
     * the screen's bottom edge and the paper rows fall below it.
     */
    const val MEADOW_END_ROW = 1670f

    /**
     * Canvas columns dropped from each side of the stretched band. The watercolour fades into paper
     * at the canvas edges, which the blurred band this replaced used to hide: measured on a
     * 1080x2400 screen, the leftmost screen column read 1.60 times the mid-frame brightness, and
     * dropping 20 columns brings it to 0.98. The right side fades over about 60 columns and is the
     * hill's own sunlit edge rather than an artifact, so it is left alone.
     */
    const val BAND_OVERSCAN_COLUMNS = 20f

    /**
     * The top of the country flag: the highest thing on the canvas that has to stay on screen.
     * Measured on the flag's own fabric (Finland rows 463 to 531), with headroom, and the other
     * flagged countries are drawn to the same template.
     */
    const val FLAG_TOP_ROW = 430f

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
        /**
         * How wide the canvas is actually drawn, and where its left edge sits. On every portrait
         * phone this is the full screen width at x 0; on a short screen the canvas is narrower and
         * centred, and the margins either side take its own edge colour.
         */
        val canvasWidth: Float,
        val canvasLeft: Float,
        val canvasHeight: Float,
        /**
         * Whether Bula and the burrow are drawn at all. False only where they cannot clear the
         * connection card, which today is a landscape phone.
         */
        val showsForeground: Boolean,
        val canvasPan: Float,
        val foregroundShift: Float,
        val landscapeTop: Float,
        val foregroundTop: Float,
        val landscapeBottom: Float,
        val foregroundBottom: Float,
        val bandLeft: Float,
        val bandWidth: Float,
        val bandHeight: Float,
    ) {
        /** The canvas row the two-part draw splits at, in screen pixels from a layer's own top. */
        val groundOffset: Float
            get() = GROUND_ROW * scale

        /** The screen row the stretched meadow starts at. */
        val bandTop: Float
            get() = foregroundTop + groundOffset
    }

    fun placement(
        screenWidth: Float,
        screenHeight: Float,
        cardTop: Float,
        density: Float,
    ): Placement {
        // Fitting the width is a MAXIMUM, not the rule. On any landscape geometry it puts the span
        // from the flag down to Bula's feet taller than the room above the card (an iPad 11 in
        // landscape wants 727 pt of the 485 it has), and the pan cap then leaves his feet below the
        // bottom edge with the flag cropped off the top anyway. So the canvas shrinks until that
        // span fits, and is centred. On every portrait phone the width term wins and nothing here
        // changes.
        val widthFit = screenWidth / CANVAS_WIDTH
        val heightFit = screenHeight / CANVAS_HEIGHT
        val room = if (cardTop.isNaN()) screenHeight else cardTop - GAP_DP * density
        val spanFit = maxOf(0f, room) / (FEET_ROW - FLAG_TOP_ROW)
        // Never below height-fit: a landscape phone leaves 84 dp above the card, and fitting the
        // span into that would draw the canvas as a 293 px strip on a 2400 px screen. The canvas
        // always covers the screen; where it then cannot clear the card, the foreground is not
        // drawn at all rather than half buried (see [Placement.showsForeground]).
        val scale = minOf(widthFit, maxOf(spanFit, heightFit))
        val drawnWidth = CANVAS_WIDTH * scale
        val canvasLeft = (screenWidth - drawnWidth) / 2f
        val canvasHeight = CANVAS_HEIGHT * scale
        val feetY = FEET_ROW * scale
        val groundY = GROUND_ROW * scale
        // Before the card has been laid out there is nothing to track, so the canvas sits where it
        // is painted rather than guessing.
        val want = if (cardTop.isNaN()) 0f else cardTop - GAP_DP * density - feetY
        // The cap is the sky above the flag, or the header band, whichever is more generous.
        // Panning by FLAG_TOP_ROW crops only rows the flag sits below, which is what that constant
        // means, so it can never hide something that has to stay in frame; on a portrait phone the
        // header band is the wider of the two and this reads as it always did.
        val panLimit = maxOf(MAX_CANVAS_PAN_DP * density, FLAG_TOP_ROW * scale)
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
        // Bula and the burrow ride the same layer, so either both clear the card or neither is
        // drawn. A landscape phone is the case that cannot: the card leaves 84 dp, and the choice
        // there is between a rabbit sunk to the ears behind it and the country art alone. The art
        // alone is a composition; the sunk rabbit is an accident.
        val cardEdge = if (cardTop.isNaN()) screenHeight else cardTop
        val showsForeground = (foregroundTop + feetY) <= cardEdge + 0.5f
        // The band is scaled so MEADOW_END_ROW lands on the screen's bottom edge; the paper rows
        // under it are drawn past that edge and clipped. It is never compressed, so on a screen the
        // canvas already covers it draws at its natural size.
        val bandNatural = canvasHeight - groundY
        val bandNeeded = maxOf(0f, screenHeight - (foregroundTop + groundY))
        val bandHeight =
            maxOf(
                bandNatural,
                bandNeeded * (CANVAS_HEIGHT - GROUND_ROW) / (MEADOW_END_ROW - GROUND_ROW),
            )
        // The band spans the drawn canvas, inset past the paper margin, so on a short screen it
        // stops with the canvas rather than running under the side margins.
        val bandScaleX = drawnWidth / (CANVAS_WIDTH - 2f * BAND_OVERSCAN_COLUMNS)
        return Placement(
            scale = scale,
            canvasWidth = drawnWidth,
            canvasLeft = canvasLeft,
            canvasHeight = canvasHeight,
            showsForeground = showsForeground,
            canvasPan = canvasPan,
            foregroundShift = foregroundShift,
            landscapeTop = canvasPan,
            foregroundTop = foregroundTop,
            // The landscape is never stretched: it is drawn whole, and everything below it is the
            // burrow layer's own opaque ground.
            landscapeBottom = canvasPan + canvasHeight,
            foregroundBottom = maxOf(foregroundTop + canvasHeight, screenHeight),
            bandLeft = canvasLeft - BAND_OVERSCAN_COLUMNS * bandScaleX,
            bandWidth = CANVAS_WIDTH * bandScaleX,
            bandHeight = bandHeight,
        )
    }
}
