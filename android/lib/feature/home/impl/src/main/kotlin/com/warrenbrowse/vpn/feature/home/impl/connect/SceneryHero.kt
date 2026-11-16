package com.warrenbrowse.vpn.feature.home.impl.connect

import android.os.Build
import android.os.SystemClock
import androidx.annotation.DrawableRes
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.AnimationSpec
import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.animation.core.EaseInOut
import androidx.compose.animation.core.EaseOut
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.graphics.BlurEffect
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.CompositingStrategy
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.RenderEffect
import androidx.compose.ui.graphics.TileMode
import androidx.compose.ui.graphics.TransformOrigin
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.clipRect
import androidx.compose.ui.graphics.drawscope.withTransform
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.repeatOnLifecycle
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.color.LocalWarrenSurfaces
import com.warrenbrowse.vpn.lib.ui.theme.tokens.DesignTokens
import kotlin.math.roundToInt
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

// The desktop CountryBackdrop literals that carry no home token: the connecting scene blurs, dims
// to brightness(0.92) and zooms 8 % over 6 s; Bula slides 3 % of the canvas down into the burrow.
internal const val BLUR_MILLIS = 900
internal const val ZOOM_MILLIS = 6000
internal const val CONNECTING_ZOOM = 1.08f
internal const val CONNECTING_DIM = 0.08f
internal const val BULA_HIDE_DROP = 0.03f

// The paper glow behind the logo: full at its corner, 43 % at 45 % of its radius, gone at 75 %.
private const val GLOW_MID_STOP = 0.45f
private const val GLOW_MID_ALPHA = 0.43f
private const val GLOW_END_STOP = 0.75f

private val hero = DesignTokens.Home.Hero

/** The desktop's easing for the hero's compact resize, cubic-bezier(0.3, 0.7, 0.2, 1). */
internal val COMPACT_EASING = CubicBezierEasing(0.3f, 0.7f, 0.2f, 1f)

// Built once: a gradient with no explicit end spans whatever it fills, so one brush serves every
// hero size, and the vararg copy is paid at class load rather than on every frame.
@Suppress("SpreadOperator")
private val HERO_MASK =
    Brush.verticalGradient(
        *SceneryFraming.MASK_STOPS.map { (at, alpha) -> at to Color.Black.copy(alpha = alpha) }
            .toTypedArray()
    )

/**
 * The home hero (desktop `CountryBackdrop`): the three registered scenery layers framed by
 * [SceneryFraming] so that what the current phase is about stays clear of the floating header
 * ([clearTop] from the top of this box) and of the fade into the panel. While connecting the
 * landscape blurs and slowly zooms; once protected Bula slips into the burrow.
 *
 * The rows above a canvas that had to drop take the landscape's top fill; the columns beside a
 * narrowed one show the same landscape at cover scale, blurred; the canvas edges facing either are
 * feathered into them. [glowTop] is the status bar the logo sits under, which its paper glow
 * reaches through. The art is framed for [frameHeight], the height the hero is heading to, so it
 * never shrinks to fit a hero halfway through its [compact] resize.
 */
@Composable
@Suppress("LongParameterList")
internal fun SceneryHero(
    phase: ConnectionPhase,
    exitCountry: String?,
    clearTop: Dp,
    glowTop: Dp,
    compact: Boolean,
    frameHeight: Dp,
    modifier: Modifier = Modifier,
) {
    val scenery = resolveScenery(phase, exitCountry)
    val landscape = SceneryFraming.landscapeOf(phase, exitCountry)
    val span = SceneryFraming.span(phase, exitCountry)
    val surfaces = LocalWarrenSurfaces.current
    WarmSceneryMasters(exitCountry)

    val topFill by
        animateColorAsState(
            targetValue = Color(landscape.topFill),
            animationSpec = tween(hero.CrossfadeMs),
            label = "hero_top_fill",
        )
    var size by remember { mutableStateOf(IntSize.Zero) }
    val density = LocalDensity.current
    val targetHeight = with(density) { frameHeight.toPx() }
    val target =
        with(density) {
            heroFrame(
                width = size.width.toFloat(),
                height = targetHeight,
                clearTop = clearTop.toPx(),
                span = span,
                compact = compact,
            )
        }
    val reframe = rememberReframeSpec(compact)
    val scrim = surfaces.homeScrim
    val glow = surfaces.homeLogoGlow

    Box(
        modifier
            .onSizeChanged { size = it }
            .clipToBounds()
            .graphicsLayer { compositingStrategy = CompositingStrategy.Offscreen }
            .drawWithContent {
                drawContent()
                drawHeroMask()
            }
            .drawBehind { drawRect(topFill) }
    ) {
        if (target != null) {
            if (target.feathersSides) {
                MarginBackdrop(
                    landscape = scenery.landscape,
                    cover =
                        SceneryFraming.backdrop(size.width.toFloat(), targetHeight, target, span),
                )
            }
            SceneryCanvas(target = target, scenery = scenery, frameSpec = reframe)
        }
        Box(
            Modifier.fillMaxSize().drawBehind {
                drawRect(
                    Brush.verticalGradient(
                        0f to scrim,
                        hero.ScrimStop to Color.Transparent,
                    )
                )
                drawLogoGlow(glow, hero.GlowWidth.toPx(), hero.GlowHeight.toPx() + glowTop.toPx())
            }
        )
    }
}

/** The frame for the measured hero, or null while it is unmeasured or has no room for the art. */
private fun heroFrame(
    width: Float,
    height: Float,
    clearTop: Float,
    span: SceneryFraming.Span,
    compact: Boolean,
): SceneryFraming.Frame? {
    val fade = SceneryFraming.fade(height)
    if (width <= 0f || height - fade - clearTop <= 0f) return null
    return SceneryFraming.frame(
        width = width,
        height = height,
        clearTop = clearTop,
        fade = fade,
        span = span,
        compact = compact,
    )
}

/**
 * How the canvas moves to a new frame: on the hero's own resize clock while the details open or
 * close, so the art and the hero edge move as one, and on the desktop's frame clock otherwise.
 */
@Composable
private fun rememberReframeSpec(compact: Boolean): AnimationSpec<Float> {
    val resize = remember { ResizeClock(compact) }
    resize.observe(compact, SystemClock.uptimeMillis())
    return if (resize.resizing(SystemClock.uptimeMillis())) {
        tween(SceneryFraming.COMPACT_TRANSITION_MS, easing = COMPACT_EASING)
    } else {
        tween(hero.FrameMs, easing = EaseInOut)
    }
}

/** Remembers when the hero last flipped between its full and its compact height. */
private class ResizeClock(private var compact: Boolean) {
    private var until = 0L

    fun observe(compact: Boolean, now: Long) {
        if (compact == this.compact) return
        this.compact = compact
        until = now + SceneryFraming.COMPACT_TRANSITION_MS
    }

    fun resizing(now: Long): Boolean = now < until
}

/**
 * The landscape behind the side margins of a narrowed canvas: the same image at cover scale,
 * blurred by [SceneryFraming.MARGIN_BLUR], so a wide hero reads as one landscape rather than a
 * picture in a flat frame. RenderEffect needs API 31; below it the margins keep the top fill.
 */
@Composable
private fun MarginBackdrop(@DrawableRes landscape: Int, cover: SceneryFraming.Frame) {
    if (Build.VERSION.SDK_INT < Build.VERSION_CODES.S) return
    val image = rememberSceneryBitmap(landscape)
    Box(
        Modifier.fillMaxSize()
            .graphicsLayer {
                val radius = SceneryFraming.MARGIN_BLUR.dp.toPx()
                renderEffect = BlurEffect(radius, radius, TileMode.Clamp)
                clip = true
            }
            .drawBehind { drawLayer(image, CanvasRect(cover.left, cover.top, cover.scale)) }
    )
}

/**
 * The registered canvas. Composed only once a frame exists, so its first frame is placed rather
 * than animated in from nowhere; every later reframe (a phase change, a new landscape) moves the
 * three layers together on the desktop's frame clock.
 */
@Composable
private fun SceneryCanvas(
    target: SceneryFraming.Frame,
    scenery: SceneryState,
    frameSpec: AnimationSpec<Float>,
) {
    val left by animateFloatAsState(target.left, frameSpec, label = "hero_canvas_left")
    val top by animateFloatAsState(target.top, frameSpec, label = "hero_canvas_top")
    val scale by animateFloatAsState(target.scale, frameSpec, label = "hero_canvas_scale")
    val feathersTop = target.feathersTop
    val feathersSides = target.feathersSides

    val blurRadius by
        animateDpAsState(
            targetValue = if (scenery.blurred) hero.BlurRadius else 0.dp,
            animationSpec = tween(BLUR_MILLIS),
            label = "hero_blur",
        )
    val zoom by
        animateFloatAsState(
            targetValue = if (scenery.blurred) CONNECTING_ZOOM else 1f,
            animationSpec = tween(ZOOM_MILLIS, easing = EaseOut),
            label = "hero_zoom",
        )
    val dim by
        animateFloatAsState(
            targetValue = if (scenery.blurred) CONNECTING_DIM else 0f,
            animationSpec = tween(BLUR_MILLIS),
            label = "hero_dim",
        )
    val bulaAlpha by
        animateFloatAsState(
            targetValue = if (scenery.showBula) 1f else 0f,
            animationSpec = tween(hero.BulaMs),
            label = "hero_bula_alpha",
        )
    val bulaDrop by
        animateFloatAsState(
            targetValue = if (scenery.showBula) 0f else BULA_HIDE_DROP,
            animationSpec = tween(hero.BulaMs),
            label = "hero_bula_drop",
        )

    val burrow = rememberSceneryBitmap(SceneryTable.burrow)
    val bula = rememberSceneryBitmap(SceneryTable.bula)
    val rect = { CanvasRect(left, top, scale) }

    Box(
        Modifier.fillMaxSize()
            .graphicsLayer { compositingStrategy = CompositingStrategy.Offscreen }
            .drawWithContent {
                drawContent()
                drawFeather(rect(), feathersTop, feathersSides)
            }
    ) {
        LandscapeScene(
            landscape = scenery.landscape,
            rect = rect,
            zoom = { zoom },
            blurRadius = { blurRadius },
            dim = { dim },
        )
        Box(
            Modifier.fillMaxSize().drawBehind {
                val r = rect()
                drawLayer(burrow, r)
                drawLayer(bula, r, dy = r.height * bulaDrop, alpha = bulaAlpha)
            }
        )
    }
}

/** Where the canvas sits in the hero, in pixels. */
private data class CanvasRect(val left: Float, val top: Float, val scale: Float) {
    val width: Float
        get() = SceneryFraming.CANVAS_WIDTH * scale

    val height: Float
        get() = SceneryFraming.CANVAS_HEIGHT * scale

    val center: Offset
        get() = Offset(left + width / 2f, top + height / 2f)
}

private fun DrawScope.drawLayer(image: ImageBitmap, r: CanvasRect, dy: Float = 0f, alpha: Float = 1f) {
    if (alpha <= 0f) return
    drawImage(
        image = image,
        srcOffset = IntOffset.Zero,
        srcSize = IntSize(image.width, image.height),
        dstOffset = IntOffset(r.left.roundToInt(), (r.top + dy).roundToInt()),
        dstSize = IntSize(r.width.roundToInt(), r.height.roundToInt()),
        alpha = alpha,
    )
}

/**
 * Fades the hero into the page along the mockup's curve ([SceneryFraming.MASK_STOPS]), the panel's
 * status riding over its last rows.
 */
private fun DrawScope.drawHeroMask() {
    drawRect(brush = HERO_MASK, blendMode = BlendMode.DstIn)
}

/** Fades the canvas edges that face the top fill: its top over a headroom strip, its sides. */
private fun DrawScope.drawFeather(r: CanvasRect, top: Boolean, sides: Boolean) {
    val feather = SceneryFraming.FEATHER_ROWS * r.scale
    if (top) {
        drawRect(
            brush =
                Brush.verticalGradient(
                    0f to Color.Transparent,
                    1f to Color.Black,
                    startY = r.top,
                    endY = r.top + feather,
                ),
            blendMode = BlendMode.DstIn,
        )
    }
    if (sides) {
        val right = r.left + r.width
        drawRect(
            brush =
                Brush.horizontalGradient(
                    0f to Color.Transparent,
                    (feather / r.width) to Color.Black,
                    (1f - feather / r.width) to Color.Black,
                    1f to Color.Transparent,
                    startX = r.left,
                    endX = right,
                ),
            blendMode = BlendMode.DstIn,
        )
    }
}

/**
 * The paper halo behind the inked logo, an ellipse anchored at the hero's top-left corner. It
 * reaches through the status bar, so it is that much taller than the desktop's; anchored below the
 * status bar instead, its top edge showed as a hard band across the sky.
 */
private fun DrawScope.drawLogoGlow(glow: Color, width: Float, height: Float) {
    if (width <= 0f) return
    withTransform({ scale(scaleX = 1f, scaleY = height / width, pivot = Offset.Zero) }) {
        drawRect(
            brush =
                Brush.radialGradient(
                    0f to glow,
                    GLOW_MID_STOP to glow.copy(alpha = glow.alpha * GLOW_MID_ALPHA),
                    GLOW_END_STOP to Color.Transparent,
                    center = Offset.Zero,
                    radius = width,
                ),
            size = Size(width, width),
        )
    }
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
 * The landscape layer, cross-faded the way the desktop does it: the outgoing image stays FULLY
 * OPAQUE underneath and only the incoming one fades in on top, so the composited opacity never
 * drops below 1. Both live in one layer, clipped to the canvas, so the connecting zoom and blur
 * take them together the way the desktop wraps them in a single Scene; the burrow and Bula stay
 * sharp above it.
 */
@Composable
private fun LandscapeScene(
    @DrawableRes landscape: Int,
    rect: () -> CanvasRect,
    zoom: () -> Float,
    blurRadius: () -> Dp,
    dim: () -> Float,
) {
    var back by remember { mutableIntStateOf(landscape) }
    var front by remember { mutableIntStateOf(landscape) }
    val frontAlpha = remember { Animatable(1f) }

    LaunchedEffect(landscape) {
        if (landscape == front) return@LaunchedEffect
        back = front
        front = landscape
        frontAlpha.snapTo(0f)
        frontAlpha.animateTo(1f, tween(hero.CrossfadeMs))
        // Fully covered now: drop the outgoing layer so nothing draws a landscape nobody sees.
        back = front
    }

    val backImage = rememberSceneryBitmap(back)
    val frontImage = rememberSceneryBitmap(front)
    val blur = remember { blurEffects() }

    Box(
        Modifier.fillMaxSize()
            .drawWithContent {
                val r = rect()
                clipRect(r.left, r.top, r.left + r.width, r.top + r.height) {
                    this@drawWithContent.drawContent()
                }
            }
            .graphicsLayer {
                val r = rect()
                if (size.width > 0f && size.height > 0f) {
                    transformOrigin =
                        TransformOrigin(r.center.x / size.width, r.center.y / size.height)
                }
                scaleX = zoom()
                scaleY = zoom()
                // A zero-radius BlurEffect is invalid; no blur means no effect.
                renderEffect = blur.effect(blurRadius().toPx())
            }
            .drawBehind {
                val r = rect()
                if (back != front) drawLayer(backImage, r)
                drawLayer(frontImage, r, alpha = frontAlpha.value)
                val d = dim()
                if (d > 0f) {
                    drawRect(
                        Color.Black,
                        topLeft = Offset(r.left, r.top),
                        size = Size(r.width, r.height),
                        alpha = d,
                    )
                }
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
internal fun rememberSceneryBitmap(@DrawableRes id: Int): ImageBitmap {
    val context = LocalContext.current
    return remember(id) { SceneryBitmaps.of(context).get(id) }
}
