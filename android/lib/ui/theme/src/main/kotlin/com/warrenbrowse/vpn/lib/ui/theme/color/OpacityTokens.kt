package com.warrenbrowse.vpn.lib.ui.theme.color

import androidx.compose.ui.graphics.Color

internal object OpacityTokens {
    // Precomputed neutral-grey blends, mirroring the desktop neutral surface +
    // text/icon ladders (color-tokens.ts). Kept hueless so the charcoal theme
    // never casts a blue/sepia tint.

    // White-on-charcoal text/icon grey.
    val WhiteOnDarkBlue60 = Color(0xFFBABABB)

    // Neutral grey surface ladder (darkest -> lightest), i.e. Blue blended over
    // the charcoal background at increasing opacity.
    val BlueOnDarkBlue10 = Color(0xFF2A2928)
    val BlueOnDarkBlue20 = Color(0xFF302F2E)
    val BlueOnDarkBlue40 = Color(0xFF383735)
    val BlueOnDarkBlue50 = Color(0xFF403E3C)
    val BlueOnDarkBlue60 = Color(0xFF484644)
    val BlueOnDarkBlue80 = Color(0xFF504E4B)
}
