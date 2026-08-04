package com.warrenbrowse.vpn.lib.ui.component

import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.width
import androidx.compose.material3.Icon
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/**
 * Warren visual identity primitives, kept in lockstep with the desktop
 * `lib/components/logo/Logo.tsx`: the ears mark (which stands in as the capital
 * W) and the "arren" wordmark beside it.
 */

// 'Exposed': Bula's masked face is out of the burrow (disconnected / default).
// 'Hidden' : Bula is safe in the burrow, only the ears show (connected).
// 'Blocked': internet blocked by the kill switch (falls back to Exposed art).
enum class WarrenLogoState { Exposed, Hidden, Blocked }

// 'Dark' mark for coloured headers (connect screen: olive / terracotta), 'Light'
// (cream) mark for the neutral charcoal headers (login / settings). Mirrors the
// desktop logoTone rule in AppMainHeader.
enum class WarrenLogoTone { Light, Dark }

// All mark PNGs share one canvas and a bottom-anchored burrow, so every state
// renders the same box (the hole stays put; only the rabbit ducks in or out).
private const val MARK_ASPECT = 968f / 687f

// The "arren" vector's viewport (Coliner Light). HEIGHT_SCALE maps the caller's
// text size onto the drawing height so it sits like the old text wordmark; tune
// here if it reads too tall or short beside the mark.
private const val WORDMARK_ASPECT = 2494f / 520f
private const val WORDMARK_HEIGHT_SCALE = 1.3f

@Composable
fun WarrenLogoMark(
    state: WarrenLogoState,
    tone: WarrenLogoTone,
    modifier: Modifier = Modifier,
    height: androidx.compose.ui.unit.Dp = 40.dp,
) {
    val drawable = when (state) {
        WarrenLogoState.Hidden ->
            if (tone == WarrenLogoTone.Light) R.drawable.logo_ears_cream else R.drawable.logo_ears
        else ->
            if (tone == WarrenLogoTone.Light) R.drawable.logo_rabbit_cream else R.drawable.logo_rabbit
    }
    Image(
        painter = painterResource(id = drawable),
        contentDescription = null, // Decorative; conveys no actionable information.
        contentScale = ContentScale.Fit,
        modifier = modifier.height(height).width(height * MARK_ASPECT),
    )
}

/**
 * "arren" wordmark drawn in Coliner Light (a vector drawable, matching desktop
 * and iOS). The ears mark rendered beside it stands in as the capital W, so the
 * word itself carries no W. [color] tints it to the header background. [fontSize]
 * keeps the old text-size call sites working, scaled to the drawing height.
 */
@Composable
fun WarrenWordmark(
    color: Color,
    modifier: Modifier = Modifier,
    fontSize: TextUnit = 22.sp,
) {
    val height = with(LocalDensity.current) { fontSize.toDp() } * WORDMARK_HEIGHT_SCALE
    Icon(
        painter = painterResource(id = R.drawable.wordmark_arren),
        contentDescription = null, // Decorative; the screen text conveys the name.
        tint = color,
        modifier = modifier.height(height).width(height * WORDMARK_ASPECT),
    )
}

// The desktop warren-wordmark.svg lockup viewport: the ears W and "arren" are
// baked as one glyph run (mirrors TopBar's WARREN_LOCKUP_ASPECT), so the brand
// reads as a single word rather than mark + text side by side.
private const val WORDMARK_LOCKUP_ASPECT = 17355f / 5720f

/**
 * Full "Warren" lockup (ears mark fused with the wordmark), used where the
 * brand should read as one word instead of the mark + [WarrenWordmark] pair,
 * e.g. the forced-update gate.
 */
@Composable
fun WarrenWordmarkLockup(
    color: Color,
    modifier: Modifier = Modifier,
    height: androidx.compose.ui.unit.Dp = 40.dp,
) {
    Icon(
        painter = painterResource(id = R.drawable.wordmark_warren),
        contentDescription = null, // Decorative; the screen text conveys the name.
        tint = color,
        modifier = modifier.height(height).width(height * WORDMARK_LOCKUP_ASPECT),
    )
}
