package com.warrenbrowse.vpn.feature.home.impl.connect

import androidx.annotation.DrawableRes
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.EaseOut
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
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
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.graphics.BlurEffect
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.RenderEffect
import androidx.compose.ui.graphics.TileMode
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.repeatOnLifecycle
import com.warrenbrowse.vpn.lib.ui.resource.R
import kotlin.math.roundToInt
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

// The connecting blur of the landscape; SceneryParityTest pins it to the desktop token.
internal val LANDSCAPE_BLUR_RADIUS = 14.dp

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

/**
 * The full-bleed illustrated backdrop of the home screen: a per-country landscape, the burrow
 * foreground and the Bula sprite, composited like the desktop `CountryBackdrop`. While connecting
 * the landscape blurs and slowly zooms ("the destination is not in focus yet"); once protected Bula
 * ducks into the burrow.
 *
 * Every layer is drawn at full screen width with no side crop, so the country flag and the burrow
 * stay in frame, and in two parts: natural down to the row where the burrow layer turns fully
 * opaque, then vertically scaled below it so the painted ground itself continues to the bottom of
 * the screen. That replaced a mirrored, blurred continuation band which read as a hard full-width
 * line and a smeared bottom quarter of the screen. The geometry is [SceneryLayout], replayed from
 * the shared fixture by every client.
 *
 * [cardTop] reports the connection card's top edge in this backdrop's own coordinates (or NaN
 * before the first layout). It is read at draw time only, so the scene follows the card's own
 * height animation frame by frame without recomposing anything.
 */
@Composable
fun SceneryBackdrop(
    phase: ConnectionPhase,
    exitCountry: String?,
    modifier: Modifier = Modifier,
    cardTop: () -> Float = { Float.NaN },
) {
    val scenery = resolveScenery(phase, exitCountry)
    WarmSceneryMasters(exitCountry)

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
    val washColor =
        animateColorAsState(
            targetValue = phase.accentColor(),
            animationSpec = tween(CROSSFADE_MILLIS),
            label = "accent_wash",
        )

    val burrow = rememberSceneryBitmap(R.drawable.scenery_terrier)
    val bula = rememberSceneryBitmap(R.drawable.scenery_bula)

    Box(modifier.fillMaxSize().background(MaterialTheme.colorScheme.surface).clipToBounds()) {
        LandscapeCrossfade(
            landscape = scenery.landscape,
            zoom = { zoom },
            blurRadius = { blurRadius },
            cardTop = cardTop,
        )
        // The connecting dim covers the landscape only: the burrow and Bula stay at full
        // brightness, as on desktop where the filter sits on the scene wrapper under them. Read at
        // draw time so the fade costs no recomposition.
        Box(Modifier.fillMaxSize().drawBehind { drawRect(Color.Black, alpha = dim) })

        // The foreground pair stays sharp and registered to EACH OTHER (Bula's shadow is painted in
        // the burrow layer). The landscape behind does not move, so the wider the gap to the card,
        // the more of the country art shows.
        Box(
            Modifier.fillMaxSize().drawBehind {
                val placement = placement(cardTop())
                drawSceneryLayer(
                    image = burrow,
                    top = placement.foregroundTop,
                    placement = placement,
                )
            }
        )
        Box(
            Modifier.fillMaxSize().drawBehind {
                val placement = placement(cardTop())
                // Bula's body ends above the split row, so his layer is never stretched.
                drawWholeCanvas(
                    image = bula,
                    top = placement.foregroundTop + size.height * bulaDrop,
                    placement = placement,
                    alpha = bulaAlpha,
                )
            }
        )

        PhaseWash { washColor.value }
    }
}

/** The placement for the current draw, from this [DrawScope]'s own size and density. */
private fun DrawScope.placement(cardTop: Float): SceneryLayout.Placement =
    SceneryLayout.placement(
        screenWidth = size.width,
        screenHeight = size.height,
        cardTop = cardTop,
        density = density,
    )

/** A layer drawn whole at its natural scale: the landscape, and Bula, who is never stretched. */
private fun DrawScope.drawWholeCanvas(
    image: ImageBitmap,
    top: Float,
    placement: SceneryLayout.Placement,
    alpha: Float = 1f,
) {
    if (alpha <= 0f) return
    drawImage(
        image = image,
        srcOffset = IntOffset.Zero,
        srcSize = IntSize(image.width, image.height),
        dstOffset = IntOffset(0, top.roundToInt()),
        dstSize = IntSize(size.width.roundToInt(), placement.canvasHeight.roundToInt()),
        alpha = alpha,
    )
}

/**
 * The burrow layer, in two parts: the rows above [SceneryLayout.GROUND_ROW] at their natural scale,
 * then the meadow below it scaled to reach the screen's bottom edge. The band is never compressed,
 * so on a screen the canvas already covers, both parts are natural and this is the plain full-width
 * draw the desktop has always done. The band is drawn a little wider than the screen and a little
 * taller than it needs: the watercolour fades into bare paper at the canvas edges, and both
 * overflows carry that paper off screen.
 */
private fun DrawScope.drawSceneryLayer(
    image: ImageBitmap,
    top: Float,
    placement: SceneryLayout.Placement,
    alpha: Float = 1f,
) {
    if (alpha <= 0f) return
    val split = SceneryLayout.GROUND_ROW.roundToInt()
    drawImage(
        image = image,
        srcOffset = IntOffset.Zero,
        srcSize = IntSize(image.width, split),
        dstOffset = IntOffset(0, top.roundToInt()),
        dstSize = IntSize(size.width.roundToInt(), placement.groundOffset.roundToInt()),
        alpha = alpha,
    )
    drawImage(
        image = image,
        srcOffset = IntOffset(0, split),
        srcSize = IntSize(image.width, image.height - split),
        dstOffset =
            IntOffset(placement.bandLeft.roundToInt(), (top + placement.groundOffset).roundToInt()),
        dstSize = IntSize(placement.bandWidth.roundToInt(), placement.bandHeight.roundToInt()),
        alpha = alpha,
    )
}

/**
 * Decodes the masters the next frames draw on IO, before any frame asks: the exit's landscape as
 * soon as the exit is known (the pin names it while disconnected), so the first connecting frame
 * finds it warm instead of decoding 7.8 MB on the main thread, and the always-drawn ones with it.
 * Run again on every return to the foreground, the pinned exit unchanged, because a memory trim
 * while the app was away drops every master.
 */
@Composable
private fun WarmSceneryMasters(exitCountry: String?) {
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
}

/**
 * The two full-bleed overlays: a faint phase-tinted wash on the top and bottom edges, blended
 * soft-light so it keys the mood to the connection state without veiling the art, then one
 * continuous bottom scrim to the very screen edge (desktop AppMainFooter) that grounds the card and
 * the footer.
 *
 * The tint is read in the draw lambda: a colour that animates for 700 ms would otherwise recompose
 * the whole backdrop on each of its frames. The gradient is rebuilt on the frames the colour moves,
 * and only those; the scrim never changes and is remembered.
 */
@Composable
private fun PhaseWash(washColor: () -> Color) {
    Box(
        Modifier.fillMaxSize().drawBehind {
            val tint = washColor()
            drawRect(
                brush =
                    Brush.verticalGradient(
                        0f to tint.copy(alpha = WASH_ALPHA),
                        WASH_TOP_STOP to Color.Transparent,
                        WASH_BOTTOM_STOP to Color.Transparent,
                        1f to tint.copy(alpha = WASH_ALPHA),
                    ),
                blendMode = WASH_BLEND,
            )
        }
    )
    val scrimBrush = remember {
        Brush.verticalGradient(
            0f to Color.Transparent,
            SCRIM_START to Color.Transparent,
            1f to Color.Black.copy(alpha = SCRIM_ALPHA),
        )
    }
    Box(Modifier.fillMaxSize().background(scrimBrush))
}

/**
 * The landscape layer, cross-faded the way the desktop does it: the outgoing image stays FULLY
 * OPAQUE underneath and only the incoming one fades in on top, so the composited opacity never
 * drops below 1. Compose's `Crossfade` animates both slots independently, which sits both layers
 * near 0.5 at the midpoint and visibly darkened the backdrop on every landscape change.
 *
 * [zoom] and [blurRadius] are read through lambdas so an animating value does not recompose this
 * body once per frame; both layers share them, so they blur and scale together.
 */
@Composable
private fun LandscapeCrossfade(
    @DrawableRes landscape: Int,
    zoom: () -> Float,
    blurRadius: () -> Dp,
    cardTop: () -> Float,
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
        // Fully covered now: drop the outgoing layer so the screen is not left drawing a full-bleed
        // image nobody can see.
        back = front
    }

    val backImage = rememberSceneryBitmap(back)
    val frontImage = rememberSceneryBitmap(front)
    val blur = remember { blurEffects() }

    // Both landscapes live in one layer so the connecting zoom and blur scale them together, the
    // way the desktop wraps them in a single transformed Scene div.
    Box(
        Modifier.fillMaxSize()
            .graphicsLayer {
                scaleX = zoom()
                scaleY = zoom()
                // A zero-radius BlurEffect is invalid; no blur means no effect.
                renderEffect = blur.effect(blurRadius().toPx())
                clip = true
            }
            .drawBehind {
                val placement = placement(cardTop())
                if (back != front) {
                    drawWholeCanvas(backImage, placement.landscapeTop, placement)
                }
                drawWholeCanvas(frontImage, placement.landscapeTop, placement, frontAlpha.value)
            }
    )
}

/** The blur effects the landscape layer cycles through, one instance per radius. */
private fun blurEffects(): RenderEffectCache<RenderEffect> = RenderEffectCache { radiusPx ->
    BlurEffect(radiusPx, radiusPx, TileMode.Decal)
}

/**
 * A scenery master from the process-wide decode cache: a cached master costs a lookup, a cold one
 * the same decode `painterResource` paid on every new composable instance.
 */
@Composable
private fun rememberSceneryBitmap(@DrawableRes id: Int): ImageBitmap {
    val context = LocalContext.current
    return remember(id) { SceneryBitmaps.of(context).get(id) }
}
