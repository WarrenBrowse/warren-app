package com.warrenbrowse.vpn.lib.ui.theme.color

import androidx.compose.ui.graphics.Color
import com.warrenbrowse.vpn.lib.ui.theme.tokens.DesignTokens

/**
 * The Warren dark palette under its historical Android names, every value an
 * alias of the generated [DesignTokens.Colors] primitive (desktop
 * `color-tokens.ts`, through `design-tokens.json`). HARD RULE from the desktop
 * source: the neutrals (background, surfaces, primary cells) are truly neutral
 * charcoal grey and carry no hue; all warmth/identity comes from the accents
 * only (olive = connected, terracotta = disconnected, ocre = brand). If the
 * neutrals lean warm the whole screen reads as a sepia wash.
 */
internal object PaletteTokens {

    // Primary interactive surface (buttons, raised cells): neutral warm-grey.
    val Blue = DesignTokens.Colors.Blue
    // Main app background / surface: neutral charcoal.
    val DarkBlue = DesignTokens.Colors.DarkBlue
    // Disconnected / error: terracotta accent.
    val Red = DesignTokens.Colors.Red
    // Connected / success: olive-green accent.
    val Green = DesignTokens.Colors.Green
    // Connecting / in-progress: orange accent, the third state of the
    // desktop tri-state (distinct from both terracotta and ocre).
    val Orange = DesignTokens.Colors.Orange

    // Lifted tints of the three phase accents, for the connect-card title
    // only: the saturated accents land near 3.5:1 on the card at title size,
    // these are built for 4.5:1. The fills keep the saturated accents.
    val RedText = DesignTokens.Colors.RedText
    val GreenText = DesignTokens.Colors.GreenText
    val OrangeText = DesignTokens.Colors.OrangeText

    // Brand warm accents (Bula), used sparingly.
    val Nose = DesignTokens.Colors.Nose // soft apricot
    val Fur = DesignTokens.Colors.Fur // ocre / beige, Bula's fur
    val Yellow = DesignTokens.Colors.Yellow // ocre (warning + brand)

    // DarkerBlue - alternative contrast surfaces such as card background.
    val DarkerBlue10 = DesignTokens.Colors.DarkerBlue10
    val DarkerBlue50 = DesignTokens.Colors.DarkerBlue50

    // White (true near-white, neutral, NOT cream).
    val MullvadWhite = DesignTokens.Colors.White
    val White = Color(0xFFFFFFFF)

    // Black
    val Black = DesignTokens.Colors.Black

    // Disabled container colors: the desktop 40 steps of the primary and the
    // two accents.
    val DisabledContainerPrimary = DesignTokens.Colors.Blue40
    val DisabledContainerTertiary = DesignTokens.Colors.Green40
    val DisabledContainerDestructive = DesignTokens.Colors.Red40
}
