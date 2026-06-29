package com.warrenbrowse.vpn.lib.ui.component

import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.width
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.ExperimentalTextApi
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontVariation
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp

/**
 * Warren visual identity primitives, kept in lockstep with the desktop
 * `lib/components/logo/Logo.tsx`: the Bula "ears-in-burrow" rabbit mark and the
 * Nunito-Black "WARREN" wordmark with a drop-cap W lettrine.
 */

// Nunito, shipped as a variable font. Black (900) is the wordmark weight; the
// explicit variationSettings pins the weight axis on API 26+ (minSdk 28) so it
// does not fall back to the font's regular default instance.
@OptIn(ExperimentalTextApi::class)
val NunitoFontFamily = FontFamily(
    Font(
        resId = R.font.nunito_variable,
        weight = FontWeight.Black,
        variationSettings = FontVariation.Settings(FontVariation.weight(900)),
    ),
)

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
 * "WARREN" wordmark: a 1.35em drop-cap W followed by "ARREN", Nunito Black.
 * [color] adapts to the header tint so it reads on every state background.
 */
@Composable
fun WarrenWordmark(
    color: Color,
    modifier: Modifier = Modifier,
    fontSize: TextUnit = 22.sp,
) {
    val text = buildAnnotatedString {
        withStyle(SpanStyle(fontSize = fontSize * 1.35f)) { append("W") }
        withStyle(SpanStyle(letterSpacing = 0.01.em)) { append("ARREN") }
    }
    Text(
        text = text,
        color = color,
        fontFamily = NunitoFontFamily,
        fontWeight = FontWeight.Black,
        fontSize = fontSize,
        maxLines = 1,
        modifier = modifier,
    )
}
