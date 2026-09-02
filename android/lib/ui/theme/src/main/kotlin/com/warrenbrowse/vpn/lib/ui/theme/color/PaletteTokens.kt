package com.warrenbrowse.vpn.lib.ui.theme.color

import androidx.compose.ui.graphics.Color

internal object PaletteTokens {

    // Warren dark palette, kept in lockstep with the desktop source of truth
    // (desktop/.../lib/foundations/tokens/color-tokens.ts). HARD RULE: the
    // neutrals (background, surfaces, primary cells) are truly neutral charcoal
    // grey and carry no hue; all warmth/identity comes from the accents only
    // (olive = connected, terracotta = disconnected, ocre = brand). If the
    // neutrals lean warm the whole screen reads as a sepia wash.

    // Primary interactive surface (buttons, raised cells): neutral warm-grey.
    val Blue = Color(0xFF4A4846)
    // Main app background / surface: neutral charcoal.
    val DarkBlue = Color(0xFF1F1F20)
    // Disconnected / error: terracotta accent.
    val Red = Color(0xFFCA4C38)
    // Connected / success: olive-green accent.
    val Green = Color(0xFF6EA24E)
    // Connecting / in-progress: orange accent, the third state of the
    // desktop tri-state (distinct from both terracotta and ocre).
    val Orange = Color(0xFFE07A28)

    // Lifted tints of the three phase accents, for the connect-card title
    // only: the saturated accents land near 3.5:1 on the card at title size,
    // these are built for 4.5:1 (desktop `redText` / `greenText` /
    // `orangeText`). The fills keep the saturated accents.
    val RedText = Color(0xFFE98E7A)
    val GreenText = Color(0xFF96C474)
    val OrangeText = Color(0xFFF0A360)

    // Brand warm accents (Bula), used sparingly.
    val Nose = Color(0xFFE8C896) // soft apricot
    val Fur = Color(0xFFD09640) // ocre / beige, Bula's fur
    val Yellow = Color(0xFFCA963C) // ocre (warning + brand)

    // DarkerBlue - alternative contrast surfaces such as card background.
    val DarkerBlue10 = Color(0xFF121213)
    val DarkerBlue50 = Color(0xFF1A1A1B)

    // White (true near-white, neutral, NOT cream).
    val MullvadWhite = Color(0xFFF7F7F8)
    val White = Color(0xFFFFFFFF)

    // Black
    val Black = Color(0xFF000000)

    // Disabled container colors: desaturated neutral / accent variants.
    // Desktop blue40 for the disabled primary fill.
    val DisabledContainerPrimary = Color(0xFF383735)
    val DisabledContainerTertiary = Color(0xFF3A5630) // olive 40
    val DisabledContainerDestructive = Color(0xFF64322C) // terracotta 40
}
