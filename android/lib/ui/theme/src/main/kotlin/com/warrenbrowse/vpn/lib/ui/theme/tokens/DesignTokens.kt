// GENERATED FILE, DO NOT EDIT.
//
// scripts/design-tokens/gen.mjs writes this object from design-tokens.json, which
// is itself derived from the desktop token sources named in the JSON. The
// desktop is the source of truth for every value; DesignTokensGateTest fails
// when this file was generated from another revision of the JSON, and
// test/unit/design-tokens.spec.ts fails when the JSON is stale against the
// desktop. Regenerate with `node scripts/design-tokens/gen.mjs`.
package com.warrenbrowse.vpn.lib.ui.theme.tokens

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/** SHA-256 of design-tokens.json at generation time. */
const val DESIGN_TOKENS_SHA256 = "2c7619a748c6c8e53949df541736774b3859dfbb024614b0137c68d562fee160"

@Suppress("MagicNumber", "unused")
object DesignTokens {
    object Colors {
        val White = Color(0xFFF7F7F8)
        val WhiteAlpha80 = Color(0xCCF7F7F8)
        val WhiteAlpha60 = Color(0x99F7F7F8)
        val WhiteAlpha40 = Color(0x66F7F7F8)
        val WhiteAlpha20 = Color(0x33F7F7F8)
        val Black = Color(0xFF000000)
        val BlackAlpha80 = Color(0xCC000000)
        val BlackAlpha60 = Color(0x99000000)
        val BlackAlpha50 = Color(0x80000000)
        val BlackAlpha40 = Color(0x66000000)
        val Red = Color(0xFFCA4C38)
        val NewRed = Color(0xFFD66046)
        val RedAlpha40 = Color(0x66CA4C38)
        val Red80 = Color(0xFFB04434)
        val Red40 = Color(0xFF64322C)
        val Green = Color(0xFF6EA24E)
        val GreenAlpha40 = Color(0x666EA24E)
        val Green80 = Color(0xFF5E8E42)
        val Green40 = Color(0xFF3A5630)
        val Orange = Color(0xFFE07A28)
        val OrangeAlpha40 = Color(0x66E07A28)
        val Orange80 = Color(0xFFC66A22)
        val RedText = Color(0xFFE98E7A)
        val GreenText = Color(0xFF96C474)
        val OrangeText = Color(0xFFF0A360)
        val Yellow = Color(0xFFCA963C)
        val Fur = Color(0xFFD09640)
        val Nose = Color(0xFFE8C896)
        val Blue = Color(0xFF4A4846)
        val DarkBlue = Color(0xFF1F1F20)
        val Dark = Color(0xFF424140)
        val DarkerBlue50 = Color(0xFF1A1A1B)
        val DarkerBlue50Alpha80 = Color(0xCC1A1A1B)
        val DarkerBlue10 = Color(0xFF121213)
        val DarkerBlue10Alpha80 = Color(0xCC121213)
        val DarkerBlue10Alpha40 = Color(0x66121213)
        val Blue10 = Color(0xFF2A2928)
        val Blue20 = Color(0xFF302F2E)
        val Blue40 = Color(0xFF383735)
        val Blue50 = Color(0xFF403E3C)
        val Blue60 = Color(0xFF484644)
        val Blue80 = Color(0xFF504E4B)
        val WhiteOnDarkBlue5 = Color(0xFF323233)
        val WhiteOnDarkBlue10 = Color(0xFF424243)
        val WhiteOnDarkBlue20 = Color(0xFF5C5C5D)
        val WhiteOnDarkBlue40 = Color(0xFF868687)
        val WhiteOnDarkBlue50 = Color(0xFF9E9E9F)
        val WhiteOnDarkBlue60 = Color(0xFFBABABB)
        val WhiteOnDarkBlue80 = Color(0xFFE4E4E5)
        val WhiteOnBlue5 = Color(0xFF4C4B49)
        val WhiteOnBlue10 = Color(0xFF585755)
        val WhiteOnBlue20 = Color(0xFF706F6C)
        val WhiteOnBlue40 = Color(0xFF969592)
        val WhiteOnBlue50 = Color(0xFFAEADAA)
        val WhiteOnBlue60 = Color(0xFFC6C5C2)
        val WhiteOnBlue80 = Color(0xFFE2E2E0)
        val Chalk = Color(0xFFF4F0E8)
        val ChalkAlpha80 = Color(0xCCF4F0E8)
        val ChalkAlpha40 = Color(0x66F4F0E8)
        val Chalk80 = Color(0xFFECE2CC)
        val Transparent = Color(0x00000000)
    }

    object Radius {
        val Radius4 = 4.dp
        val Radius8 = 8.dp
        val Radius12 = 12.dp
        val Radius16 = 16.dp
        val Radius20 = 20.dp
        val Radius24 = 24.dp
        val Radius32 = 32.dp
        val Radius48 = 48.dp
        val RadiusFull = 1000.dp
    }

    object Spacing {
        val Spc4 = 4.dp
        val Spc8 = 8.dp
        val Spc16 = 16.dp
        val Spc24 = 24.dp
        val Spc32 = 32.dp
    }

    object FontFamilies {
        const val OpenSans = "Open Sans"
        const val SourceSansPro = "Source Sans Pro"
        const val Nunito = "Nunito"
    }

    object FontWeights {
        const val Regular = 400
        const val SemiBold = 600
        const val Bold = 700
    }

    object FontSizes {
        val Big = 32.sp
        val Large = 24.sp
        val Medium = 18.sp
        val Small = 14.sp
        val Tiny = 12.sp
        val Mini = 10.sp
    }

    object LineHeights {
        val Big = 34.sp
        val Large = 28.sp
        val Medium = 24.sp
        val Small = 20.sp
        val Tiny = 18.sp
        val Mini = 15.sp
    }

    object ConnectionCard {
        val PaddingVertical = 14.dp
        val PaddingHorizontal = 16.dp
        val Radius = 16.dp
        const val SurfaceColor = "darkerBlue50Alpha80"
        const val SurfaceAlpha = 0.8f
        val BorderWidth = 1.dp
        const val BorderAlpha = 0.2f
        val RailWidth = 3.dp
        val BadgeGap = 5.dp
        val BadgesToCardGap = 8.dp
        val ButtonGap = 12.dp
        const val Transition = 300
    }

    object ConnectionStatus {
        val RowGap = 12.dp
        val WellSize = 36.dp
        val WellRadius = 11.dp
        const val WellFillAlpha = 0.22f
        const val WellBorderAlpha = 0.45f
        const val WellTransition = 300
        val IconSize = 18.dp
        val TitleSize = 19.sp
        val TitleLineHeight = 22.sp
        val SubtitleSize = 13.sp
        val SubtitleLineHeight = 18.sp
        const val SubtitleAlpha = 0.8f
    }

    object FeatureChip {
        val PaddingVertical = 2.dp
        val PaddingHorizontal = 8.dp
        val Radius = 8.dp
        val BorderWidth = 1.dp
        const val FillColor = "blue10"
        const val BorderColor = "blue"
        const val ErrorFillColor = "redAlpha40"
        const val ErrorFillAlpha = 0.4f
    }

    object CountryFlag {
        val Size = 22.dp
        val BorderWidth = 1.dp
        const val BorderAlpha = 0.2f
    }

    object Footer {
        val PaddingVertical = 7.dp
        val PaddingHorizontal = 16.dp
        const val SurfaceAlpha = 0.6f
        val BorderWidth = 1.dp
        const val BorderAlpha = 0.2f
    }

    object NotificationBanner {
        val MaxWidth = 300.dp
        val Radius = 14.dp
        val EdgeWidth = 2.dp
        const val EdgeColor = "green"
        const val SurfaceAlpha = 0.6f
        val MarginTop = 12.dp
        val MarginEnd = 16.dp
        val PaddingVertical = 10.dp
        val PaddingStart = 16.dp
        val PaddingEnd = 12.dp
        val Elevation = 8.dp
        const val Transition = 250
    }

    object Scenery {
        val BlurRadius = 14.dp
        const val ConnectingBrightness = 0.92f
        const val ConnectingZoom = 1.08f
        const val BlurTransition = 900
        const val ZoomTransition = 6000
        const val Crossfade = 700
        const val BulaTransition = 550
        const val BulaHideDrop = 0.03f
        const val WashAlpha = 0.14f
        const val WashBlend = "soft-light"
        const val WashTopStop = 0.22f
        const val WashBottomStop = 0.78f
        const val WashTransition = 700
    }

    object Navigation {
        const val Duration = 450
        const val PushNewFrom = 1f
        const val PushOldTo = -0.33f
    }
}
