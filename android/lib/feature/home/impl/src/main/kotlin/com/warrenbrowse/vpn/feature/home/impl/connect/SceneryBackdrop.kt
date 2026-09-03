package com.warrenbrowse.vpn.feature.home.impl.connect

import androidx.annotation.DrawableRes
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.EaseOut
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.graphics.BlurEffect
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.CompositingStrategy
import androidx.compose.ui.graphics.RenderEffect
import androidx.compose.ui.graphics.TileMode
import androidx.compose.ui.graphics.TransformOrigin
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.painter.BitmapPainter
import androidx.compose.ui.graphics.painter.Painter
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.layout.Layout
import androidx.compose.ui.layout.layout
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.repeatOnLifecycle
import com.warrenbrowse.vpn.lib.ui.resource.R
import kotlin.math.roundToInt
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

// The scenery masters are one pre-registered 1140x1706 canvas per layer
// (landscape, burrow, Bula). Drawing every layer full width and top anchored
// shows the WHOLE canvas (no side crop: flags and landmarks at the edges stay
// in frame) and keeps the layers registered exactly as painted. On a phone the
// canvas ends around two thirds down the screen, right above the connection
// card; the space below is filled by a blurred continuation of the same art.
private const val CANVAS_RATIO = 1706f / 1140f

// Feather height over which the sharp canvas dissolves into the blurred
// continuation, so the seam never reads as a hard edge.
private val SEAM_FEATHER = 56.dp

// The continuation below the canvas is the canvas itself mirrored around its
// bottom edge and blurred: a reflection continues the exact colors it meets,
// so the seam cannot band whatever the art.
private val CONTINUATION_BLUR_RADIUS = 28.dp

// The continuation's blur layer is cut down to the rows of the canvas the
// mirror actually shows below the seam, plus this much above them. The
// layer's edge treatment is Decal (a transparent outside) and a Gaussian of
// radius r fades about 1.5 r inward from an edge, so the top rows of the
// band fade; the margin, twice the largest radius, puts that fade past the
// bottom of the screen once the band is mirrored, and every visible pixel
// is the one the full-screen layer produced.
private val CONTINUATION_BAND_MARGIN = 84.dp

// Bula's feet row in the 1140x1706 masters (alpha bounding box of
// scenery_bula). The foreground pair (burrow + Bula, registered to each other,
// shadow included) slides vertically so this line always sits just above the
// connection card: right above the card while disconnected, and never under it
// when the card grows.
private const val BULA_FEET_FRACTION = 1332f / 1706f

// Air kept between Bula's feet and the card's top edge.
private val FOREGROUND_CARD_GAP = 16.dp

// Hiding slides Bula 3% of the height down into the burrow while fading.
internal const val BULA_HIDE_DROP = 0.03f

// Animation timings, matching the desktop CSS transitions (CountryBackdrop);
// SceneryParityTest pins them to the generated desktop tokens.
internal const val CROSSFADE_MILLIS = 700
internal const val BLUR_MILLIS = 900
internal const val ZOOM_MILLIS = 6000
internal const val BULA_MILLIS = 550

internal const val CONNECTING_ZOOM = 1.08f
// The desktop dims the connecting landscape to brightness(0.92); over opaque
// art that is a black overlay at 8 %, animated on the blur's own clock.
internal const val CONNECTING_DIM = 0.08f
internal const val WASH_ALPHA = 0.14f
internal const val WASH_TOP_STOP = 0.22f
internal const val WASH_BOTTOM_STOP = 0.78f
// Desktop `mix-blend-mode: soft-light`: the tint keys the mood without a flat
// veil over the art. Below API 29 the canvas has no soft-light and the wash
// composites as plain alpha, the same overlay as before.
internal val WASH_BLEND = BlendMode.Softlight
private const val SCRIM_START = 0.66f
private const val SCRIM_ALPHA = 0.6f
internal val LANDSCAPE_BLUR_RADIUS = 14.dp

/**
 * The full-bleed illustrated backdrop of the home screen: a per-country
 * landscape, the burrow foreground and the Bula sprite, composited like the
 * desktop `CountryBackdrop` but framed the way the iOS client frames it: the
 * whole master canvas visible at full width, top anchored, continued downward
 * by a blurred band. While connecting the landscape blurs and slowly zooms
 * ("the destination is not in focus yet"); once protected Bula ducks into the
 * burrow.
 *
 * [cardTop] reports the connection card's top edge in this backdrop's own
 * coordinates (or NaN before the first layout). It is read at draw time only,
 * so the burrow follows the card's own height animation frame by frame without
 * recomposing anything.
 */
@Composable
fun SceneryBackdrop(
    phase: ConnectionPhase,
    exitCountry: String?,
    modifier: Modifier = Modifier,
    cardTop: () -> Float = { Float.NaN },
) {
    val scenery = resolveScenery(phase, exitCountry)

    // The exit's landscape is decoded on IO as soon as the exit is known (the
    // pin names it while disconnected), so the first connecting frame finds it
    // warm instead of decoding 7.8 MB on the main thread; and again on every
    // return to the foreground, the pinned exit unchanged, because a memory
    // trim while the app was away drops every master. The always-drawn ones
    // are re-warmed for the same reason.
    val context = LocalContext.current
    val lifecycle = LocalLifecycleOwner.current.lifecycle
    LaunchedEffect(exitCountry, lifecycle) {
        lifecycle.repeatOnLifecycle(Lifecycle.State.STARTED) {
            withContext(Dispatchers.IO) {
                val bitmaps = SceneryBitmaps.of(context)
                firstFrameMasters().forEach(bitmaps::warm)
                bitmaps.warm(countryLandscape(exitCountry))
            }
        }
    }

    val blurRadius by
        animateDpAsState(
            targetValue = if (scenery.blurred) LANDSCAPE_BLUR_RADIUS else 0.dp,
            animationSpec = tween(BLUR_MILLIS),
            label = "scenery_blur",
        )
    val zoom by
        animateFloatAsState(
            targetValue = if (scenery.blurred) CONNECTING_ZOOM else 1f,
            animationSpec = tween(ZOOM_MILLIS, easing = EaseOut),
            label = "scenery_zoom",
        )
    val dim by
        animateFloatAsState(
            targetValue = if (scenery.blurred) CONNECTING_DIM else 0f,
            animationSpec = tween(BLUR_MILLIS),
            label = "scenery_dim",
        )
    val bulaAlpha by
        animateFloatAsState(
            targetValue = if (scenery.showBula) 1f else 0f,
            animationSpec = tween(BULA_MILLIS),
            label = "bula_alpha",
        )
    val bulaDrop by
        animateFloatAsState(
            targetValue = if (scenery.showBula) 0f else BULA_HIDE_DROP,
            animationSpec = tween(BULA_MILLIS),
            label = "bula_drop",
        )
    val washColor by
        animateColorAsState(
            targetValue = phase.accentColor(),
            animationSpec = tween(CROSSFADE_MILLIS),
            label = "accent_wash",
        )

    Box(modifier.fillMaxSize().background(MaterialTheme.colorScheme.surface).clipToBounds()) {
        LandscapeCrossfade(
            landscape = scenery.landscape,
            zoom = { zoom },
            blurRadius = { blurRadius },
        )
        // The connecting dim covers the landscape only: the burrow and Bula
        // stay at full brightness, as on desktop where the filter sits on the
        // scene wrapper under them. Read at draw time so the fade costs no
        // recomposition.
        Box(Modifier.fillMaxSize().drawBehind { drawRect(Color.Black, alpha = dim) })

        // Foreground layers stay sharp and registered to EACH OTHER (Bula's
        // shadow is painted in the burrow layer), drawn at the same full-width
        // scale as the landscape and slid together so Bula's feet track the
        // card's top edge: right above the card while disconnected, still in
        // view when the card grows. The landscape behind does not move, so the
        // wider the gap to the card, the more of the country art shows.
        Image(
            painter = rememberSceneryPainter(R.drawable.scenery_terrier),
            contentDescription = null, // Decorative backdrop.
            contentScale = ContentScale.FillWidth,
            alignment = Alignment.TopCenter,
            modifier =
                Modifier.fillMaxSize().graphicsLayer {
                    translationY = foregroundShift(cardTop())
                },
        )
        Image(
            painter = rememberSceneryPainter(R.drawable.scenery_bula),
            contentDescription = null, // Decorative backdrop.
            contentScale = ContentScale.FillWidth,
            alignment = Alignment.TopCenter,
            modifier =
                Modifier.fillMaxSize().graphicsLayer {
                    alpha = bulaAlpha
                    translationY = foregroundShift(cardTop()) + size.height * bulaDrop
                },
        )

        PhaseWash(washColor)
    }
}

/**
 * Vertical translation of the foreground pair so Bula's painted feet line
 * lands [FOREGROUND_CARD_GAP] above the card's top edge. Positive when the
 * card sits low (disconnected: the pair slides down, revealing more
 * landscape), smaller or negative as the card grows so the burrow mouth never
 * disappears under it. Zero before the first card layout.
 */
private fun androidx.compose.ui.graphics.GraphicsLayerScope.foregroundShift(
    cardTopPx: Float
): Float {
    if (cardTopPx.isNaN()) return 0f
    val feetY = size.width * CANVAS_RATIO * BULA_FEET_FRACTION
    return cardTopPx - FOREGROUND_CARD_GAP.toPx() - feetY
}

/**
 * The two full-bleed overlays: a faint phase-tinted wash on the top and bottom
 * edges, blended soft-light so it keys the mood to the connection state
 * without veiling the art, then one continuous bottom scrim to the very screen
 * edge (desktop AppMainFooter) that grounds the card and footer over the
 * blurred continuation band.
 *
 * Both brushes are memoized: a full-screen gradient reallocated on every frame
 * of the 700ms wash is pure garbage, and the scrim never changes at all.
 */
@Composable
private fun PhaseWash(washColor: Color) {
    val washBrush =
        remember(washColor) {
            Brush.verticalGradient(
                0f to washColor.copy(alpha = WASH_ALPHA),
                WASH_TOP_STOP to Color.Transparent,
                WASH_BOTTOM_STOP to Color.Transparent,
                1f to washColor.copy(alpha = WASH_ALPHA),
            )
        }
    Box(Modifier.fillMaxSize().drawBehind { drawRect(brush = washBrush, blendMode = WASH_BLEND) })
    val scrimBrush =
        remember {
            Brush.verticalGradient(
                0f to Color.Transparent,
                SCRIM_START to Color.Transparent,
                1f to Color.Black.copy(alpha = SCRIM_ALPHA),
            )
        }
    Box(Modifier.fillMaxSize().background(scrimBrush))
}

/**
 * The landscape layer, cross-faded the way the desktop does it: the outgoing
 * image stays FULLY OPAQUE underneath and only the incoming one fades in on
 * top, so the composited opacity never drops below 1. Compose's `Crossfade`
 * animates both slots independently, which sits both layers near 0.5 at the
 * midpoint and visibly darkened the backdrop on every landscape change.
 *
 * [zoom] and [blurRadius] are read through lambdas so an animating value does
 * not recompose this body once per frame; both layers share them, so they blur
 * and scale together.
 */
@Composable
private fun LandscapeCrossfade(
    @DrawableRes landscape: Int,
    zoom: () -> Float,
    blurRadius: () -> Dp,
) {
    var back by remember { mutableIntStateOf(landscape) }
    var front by remember { mutableIntStateOf(landscape) }
    val frontAlpha = remember { Animatable(1f) }

    LaunchedEffect(landscape) {
        if (landscape == front) return@LaunchedEffect
        back = front
        front = landscape
        frontAlpha.snapTo(0f)
        frontAlpha.animateTo(1f, tween(CROSSFADE_MILLIS))
        // Fully covered now: drop the outgoing layer so the screen is not left
        // drawing a full-bleed image nobody can see.
        back = front
    }

    if (back != front) {
        Landscape(landscape = back, zoom = zoom, blurRadius = blurRadius)
    }
    Landscape(
        landscape = front,
        zoom = zoom,
        blurRadius = blurRadius,
        alpha = { frontAlpha.value },
    )
}

/**
 * One landscape: the blurred continuation band underneath, then the sharp
 * full-width canvas on top, its bottom edge feathered into the continuation
 * so the seam never shows. Every animated value is read INSIDE a
 * graphicsLayer or draw lambda (draw phase), so composition does not re-run
 * for a single frame of the blur, the zoom or the fade, and the two blur
 * effects are reused across frames while their radius holds.
 */
@Composable
private fun Landscape(
    @DrawableRes landscape: Int,
    zoom: () -> Float,
    blurRadius: () -> Dp,
    alpha: () -> Float = { 1f },
) {
    val painter = rememberSceneryPainter(landscape)
    val continuationBlur = remember { blurEffects() }
    val canvasBlur = remember { blurEffects() }
    // Continuation band: the canvas mirrored around its own bottom edge, then
    // blurred hard. A reflection meets the seam with the very colors the sharp
    // canvas ends on, so no banding is possible whatever the art. The layer
    // that carries the blur, the mirror and the zoom covers only the rows of
    // the canvas the mirror shows below the seam, so the ~110 px Gaussian
    // costs that band's pixels per frame instead of the whole screen's. The
    // transforms stay on the blurred layer itself, as they were on the
    // full-screen one: the blur runs on the unscaled band and its edge fade is
    // scaled outward with it, which is what keeps the screen edges identical.
    Layout(
        content = {
            Box(
                Modifier.graphicsLayer {
                        this.alpha = alpha()
                        // The band's bottom edge is the seam.
                        transformOrigin = TransformOrigin(0.5f, 1f)
                        scaleX = zoom()
                        scaleY = -zoom()
                        val radiusPx = CONTINUATION_BLUR_RADIUS.toPx() + blurRadius().toPx()
                        renderEffect = continuationBlur.effect(radiusPx)
                        clip = true
                    }
                    .continuationBand()
            ) {
                Image(
                    painter = painter,
                    contentDescription = null, // Decorative backdrop.
                    contentScale = ContentScale.FillWidth,
                    alignment = Alignment.TopCenter,
                    modifier = Modifier.fillMaxSize(),
                )
            }
        },
        modifier = Modifier.fillMaxSize(),
    ) { measurables, constraints ->
        // Loose constraints: a reported size is coerced into the constraints a
        // node was measured with, so the band could never be shorter than the
        // screen under the fixed ones this full-size layout receives.
        val band =
            measurables.single().measure(constraints.copy(minWidth = 0, minHeight = 0))
        val seam = (constraints.maxWidth * CANVAS_RATIO).roundToInt()
        layout(constraints.maxWidth, constraints.maxHeight) { band.place(0, seam - band.height) }
    }
    Image(
        painter = painter,
        contentDescription = null, // Decorative backdrop.
        contentScale = ContentScale.FillWidth,
        alignment = Alignment.TopCenter,
        modifier =
            Modifier.fillMaxSize()
                .graphicsLayer {
                    this.alpha = alpha()
                    scaleX = zoom()
                    scaleY = zoom()
                    // A zero-radius BlurEffect is invalid; no blur means no effect.
                    renderEffect = canvasBlur.effect(blurRadius().toPx())
                    // Offscreen so the DstIn feather below cuts this layer's own
                    // pixels rather than everything already on screen.
                    compositingStrategy = CompositingStrategy.Offscreen
                    clip = true
                }
                .drawWithContent {
                    drawContent()
                    val canvasBottom = size.width * CANVAS_RATIO
                    val featherPx = SEAM_FEATHER.toPx()
                    drawRect(
                        brush =
                            Brush.verticalGradient(
                                colors = listOf(Color.Black, Color.Transparent),
                                startY = canvasBottom - featherPx,
                                endY = canvasBottom,
                            ),
                        topLeft = Offset(0f, canvasBottom - featherPx),
                        size =
                            androidx.compose.ui.geometry.Size(
                                size.width,
                                featherPx,
                            ),
                        blendMode = BlendMode.DstIn,
                    )
                },
    )
}

/** The blur effects a landscape layer cycles through, one instance per radius. */
private fun blurEffects(): RenderEffectCache<RenderEffect> =
    RenderEffectCache { radiusPx -> BlurEffect(radiusPx, radiusPx, TileMode.Decal) }

/**
 * Height of the continuation band's source, in pixels, for a screen
 * [widthPx] wide and [heightPx] tall: the rows above the seam that the
 * mirror shows below it, plus the blur margin.
 */
internal fun continuationBandHeight(widthPx: Float, heightPx: Float, density: Density): Float =
    with(density) {
        val seam = widthPx * CANVAS_RATIO
        (heightPx - seam).coerceAtLeast(0f) + CONTINUATION_BAND_MARGIN.toPx()
    }

/**
 * Sizes the node to the continuation band's source rows, the ones just
 * above the seam, while measuring its content full screen and sliding it up
 * so those rows fill the node. The parent places the node with its bottom
 * edge on the seam, and the node's own transform mirrors it below.
 */
private fun Modifier.continuationBand(): Modifier = layout { measurable, constraints ->
    val placeable = measurable.measure(constraints)
    val seam = (placeable.width * CANVAS_RATIO).roundToInt()
    val height =
        continuationBandHeight(placeable.width.toFloat(), placeable.height.toFloat(), this)
            .roundToInt()
            .coerceIn(0, seam)
    layout(placeable.width, height) { placeable.place(0, height - seam) }
}

/**
 * The painter for a scenery master, from the process-wide decode cache: a
 * cached master costs a lookup, a cold one the same decode `painterResource`
 * paid on every new composable instance.
 */
@Composable
private fun rememberSceneryPainter(@DrawableRes id: Int): Painter {
    val context = LocalContext.current
    return remember(id) { BitmapPainter(SceneryBitmaps.of(context).get(id)) }
}
