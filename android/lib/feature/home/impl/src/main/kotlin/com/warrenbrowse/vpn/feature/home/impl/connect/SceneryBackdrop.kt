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
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.graphics.BlurEffect
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.CompositingStrategy
import androidx.compose.ui.graphics.TileMode
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.warrenbrowse.vpn.lib.ui.resource.R

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

// Bula's feet row in the 1140x1706 masters (alpha bounding box of
// scenery_bula). The foreground pair (burrow + Bula, registered to each other,
// shadow included) slides vertically so this line always sits just above the
// connection card: right above the card while disconnected, and never under it
// when the card grows.
private const val BULA_FEET_FRACTION = 1316f / 1706f

// Air kept between Bula's feet and the card's top edge.
private val FOREGROUND_CARD_GAP = 16.dp

// Hiding slides Bula 3% of the height down into the burrow while fading.
private const val BULA_HIDE_DROP = 0.03f

// Animation timings, matching the desktop CSS transitions.
private const val CROSSFADE_MILLIS = 700
private const val BLUR_MILLIS = 900
private const val ZOOM_MILLIS = 6000
private const val BULA_MILLIS = 550

private const val CONNECTING_ZOOM = 1.08f
private const val WASH_ALPHA = 0.14f
private const val SCRIM_START = 0.66f
private const val SCRIM_ALPHA = 0.6f
private val LANDSCAPE_BLUR_RADIUS = 14.dp

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

        // Foreground layers stay sharp and registered to EACH OTHER (Bula's
        // shadow is painted in the burrow layer), drawn at the same full-width
        // scale as the landscape and slid together so Bula's feet track the
        // card's top edge: right above the card while disconnected, still in
        // view when the card grows. The landscape behind does not move, so the
        // wider the gap to the card, the more of the country art shows.
        Image(
            painter = painterResource(id = R.drawable.scenery_terrier),
            contentDescription = null, // Decorative backdrop.
            contentScale = ContentScale.FillWidth,
            alignment = Alignment.TopCenter,
            modifier =
                Modifier.fillMaxSize().graphicsLayer {
                    translationY = foregroundShift(cardTop())
                },
        )
        Image(
            painter = painterResource(id = R.drawable.scenery_bula),
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
 * edges (enough to key the mood to the connection state without washing out the
 * art), then one continuous bottom scrim to the very screen edge (desktop
 * AppMainFooter) that grounds the card and footer over the blurred
 * continuation band.
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
                0.22f to Color.Transparent,
                0.78f to Color.Transparent,
                1f to washColor.copy(alpha = WASH_ALPHA),
            )
        }
    Box(Modifier.fillMaxSize().background(washBrush))
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
 * One landscape: the blurred full-screen continuation underneath, then the
 * sharp full-width canvas on top, its bottom edge feathered into the
 * continuation so the seam never shows. Every animated value is read INSIDE a
 * graphicsLayer or draw lambda (draw phase), so composition does not re-run
 * for a single frame of the blur, the zoom or the fade.
 */
@Composable
private fun Landscape(
    @DrawableRes landscape: Int,
    zoom: () -> Float,
    blurRadius: () -> Dp,
    alpha: () -> Float = { 1f },
) {
    val painter = painterResource(id = landscape)
    // Continuation band: the canvas mirrored around its own bottom edge, then
    // blurred hard. A reflection meets the seam with the very colors the sharp
    // canvas ends on, so no banding is possible whatever the art.
    Image(
        painter = painter,
        contentDescription = null, // Decorative backdrop.
        contentScale = ContentScale.FillWidth,
        alignment = Alignment.TopCenter,
        modifier =
            Modifier.fillMaxSize().graphicsLayer {
                this.alpha = alpha()
                val canvasBottomFraction = (size.width * CANVAS_RATIO) / size.height
                transformOrigin =
                    androidx.compose.ui.graphics.TransformOrigin(0.5f, canvasBottomFraction)
                scaleX = zoom()
                scaleY = -zoom()
                val radiusPx = CONTINUATION_BLUR_RADIUS.toPx() + blurRadius().toPx()
                renderEffect = BlurEffect(radiusPx, radiusPx, TileMode.Decal)
                clip = true
            },
    )
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
                    val radiusPx = blurRadius().toPx()
                    // A zero-radius BlurEffect is invalid; no blur means no effect.
                    renderEffect =
                        if (radiusPx > 0f) BlurEffect(radiusPx, radiusPx, TileMode.Decal) else null
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
