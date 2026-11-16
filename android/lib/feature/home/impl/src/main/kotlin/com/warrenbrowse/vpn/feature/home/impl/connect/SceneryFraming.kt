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

    /** The mockup's proportions (Warren VPN Accueil, 380x620): a 300 high hero on a 380 width. */
    const val HERO_HEIGHT_RATIO = 300f / 380f

    /** A phone hero takes free height up to this fraction of its width, which width-fit covers. */
    const val HERO_MAX_HEIGHT_RATIO = 1.4f

    /** The hero while the connection details are open (176 on the 380 width). */
    const val COMPACT_HEIGHT_RATIO = 176f / 380f

    /** The hero's resize when the details open or close, on the desktop easing cubic-bezier(0.3, 0.7, 0.2, 1). */
    const val COMPACT_TRANSITION_MS = 400

    /**
     * The blur, in dp, of the landscape shown at cover scale beside a narrowed canvas: an ambient
     * wash with no recognisable shape.
     */
    const val MARGIN_BLUR = 60f

    /**
     * The hero's alpha from its top to its bottom, the mockup's soft fade: `(fraction of the hero
     * height, alpha)`. The status pill rides over the last rows.
     */
    val MASK_STOPS: List<Pair<Float, Float>> =
        listOf(
            0f to 1f,
            0.74f to 1f,
            0.79f to 0.9f,
            0.84f to 0.7f,
            0.89f to 0.45f,
            0.94f to 0.2f,
            0.98f to 0.06f,
            1f to 0f,
        )

    /** Where [MASK_STOPS] reaches half opacity: the clear zone ends there. */
    const val CLEAR_BOTTOM_FRACTION = 0.88f

    /** The cameras, the burrow and Bula down to his tail while he is out. */
    val GROUND_SPAN = Span(top = 320f, bottom = 900f)

    /** Once Bula is in: the skyline down to below the foot of its flag. */
    const val SKY_BOTTOM_ROW = 680f

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

    /**
     * Where the canvas sits in a [width] x [height] hero. A [compact] hero (the details are open)
     * keeps the scale the same hero has at its [fullHeight] and pins the span top under the
     * header, its bottom going into the fade: opening the details slides the art up and never
     * zooms into it.
     */
    @Suppress("LongParameterList")
    fun frame(
        width: Float,
        height: Float,
        clearTop: Float,
        fade: Float,
        span: Span,
        compact: Boolean = false,
        fullHeight: Float? = null,
    ): Frame {
        val clearBottom = height - fade
        val fitBottom = if (compact) (fullHeight ?: height) * CLEAR_BOTTOM_FRACTION else clearBottom
        val scale = minOf(width / CANVAS_WIDTH, (fitBottom - clearTop) / (span.bottom - span.top))
        val yMin = clearTop - span.top * scale
        val yMax = clearBottom - span.bottom * scale
        val top = if (compact) yMin else minOf(yMax, maxOf(yMin, 0f))
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

    /** The band under the clear zone, for a hero [height] tall. */
    fun fade(height: Float): Float = height * (1f - CLEAR_BOTTOM_FRACTION)

    /**
     * A phone hero [width] wide under the [statusBar]: it absorbs the [available] height the panel
     * and the footer leave, from the mockup's ratio of its width up to [HERO_MAX_HEIGHT_RATIO]; a
     * screen too short for the mockup's ratio gives it less, down to the compact hero, below which
     * the content scrolls instead.
     */
    fun heroHeight(width: Float, statusBar: Float, available: Float): Float =
        available.coerceIn(
            compactHeroHeight(width, statusBar),
            width * HERO_MAX_HEIGHT_RATIO + statusBar,
        )

    /** The hero while the details are open: [COMPACT_HEIGHT_RATIO] of its width. */
    fun compactHeroHeight(width: Float, statusBar: Float): Float =
        width * COMPACT_HEIGHT_RATIO + statusBar

    /**
     * The landscape behind the side margins of a narrowed [frame]: the canvas at the scale that
     * covers the whole hero, blurred by [MARGIN_BLUR], its [span] centred on the same row as the
     * framed canvas so the horizon runs on behind it.
     */
    fun backdrop(width: Float, height: Float, frame: Frame, span: Span): Frame {
        val scale = maxOf(width / CANVAS_WIDTH, height / CANVAS_HEIGHT)
        val middle = (span.top + span.bottom) / 2f
        val top =
            (frame.top + middle * frame.scale - middle * scale).coerceIn(
                height - CANVAS_HEIGHT * scale,
                0f,
            )
        return Frame(
            scale = scale,
            left = (width - CANVAS_WIDTH * scale) / 2f,
            top = top,
            width = CANVAS_WIDTH * scale,
            height = CANVAS_HEIGHT * scale,
            headroom = 0f,
        )
    }
}
