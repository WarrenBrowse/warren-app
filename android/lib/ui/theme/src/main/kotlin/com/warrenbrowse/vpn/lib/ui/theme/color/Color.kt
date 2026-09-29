package com.warrenbrowse.vpn.lib.ui.theme.color

import androidx.compose.material3.ColorScheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.MenuDefaults
import androidx.compose.material3.MenuItemColors
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color

const val AlphaVisible = 1f
const val Alpha5 = 0.05f
const val Alpha10 = 0.1f
const val AlphaDisabled = 0.2f
const val Alpha20 = 0.2f
const val AlphaInactive = 0.4f
const val Alpha40 = 0.4f
const val Alpha60 = 0.6f
const val AlphaDisconnectButton = 0.6f
const val AlphaScrollbar = 0.6f
const val AlphaInvisible = 0f
const val Alpha80 = 0.8f

// Special colors
// Static defined positive/success/selected color
val ColorScheme.positive: Color
    @Composable get() = PaletteTokens.Green

val ColorScheme.onSelected: Color
    @Composable get() = MaterialTheme.colorScheme.onTertiary

// Static defined warning color
val ColorScheme.warning: Color
    @Composable get() = PaletteTokens.Yellow

// The brand ocre (desktop `fur`): the one accent that says nothing about the
// tunnel state, for the marks that must not read as one (the forum badge).
val ColorScheme.brand: Color
    @Composable get() = PaletteTokens.Fur

// Static defined connecting/in-progress color (desktop `orange` accent)
val ColorScheme.pending: Color
    @Composable get() = PaletteTokens.Orange

// Lifted tints of the three accents for text at title size (desktop
// `greenText` / `orangeText` / `redText`): the fills stay saturated, the
// title needs the contrast.
val ColorScheme.positiveText: Color
    @Composable get() = PaletteTokens.GreenText

val ColorScheme.pendingText: Color
    @Composable get() = PaletteTokens.OrangeText

val ColorScheme.errorText: Color
    @Composable get() = PaletteTokens.RedText

// Disabled colors for buttons
val ColorScheme.tertiaryDisabled: Color
    @Composable get() = PaletteTokens.DisabledContainerTertiary

val ColorScheme.primaryDisabled: Color
    @Composable get() = PaletteTokens.DisabledContainerPrimary

val ColorScheme.errorDisabled: Color
    @Composable get() = PaletteTokens.DisabledContainerDestructive

val menuItemColors: MenuItemColors
    @Composable
    get() =
        MenuDefaults.itemColors()
            .copy(
                leadingIconColor = MaterialTheme.colorScheme.onSurface,
                textColor = MaterialTheme.colorScheme.onSurface,
            )
