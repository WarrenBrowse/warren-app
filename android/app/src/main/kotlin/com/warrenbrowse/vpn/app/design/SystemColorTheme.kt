package com.warrenbrowse.vpn.app.design

import android.content.res.Configuration
import com.warrenbrowse.vpn.lib.model.ColorTheme
import com.warrenbrowse.vpn.lib.ui.theme.color.WarrenSurfaces

/**
 * The theme the device states in its [uiMode], or null when it states none. Compose's
 * `isSystemInDarkTheme` would read "no statement" as light; the house palette for that case is
 * dark, so the distinction is kept for [com.warrenbrowse.vpn.lib.model.ThemePreference.resolve].
 */
fun systemColorTheme(uiMode: Int): ColorTheme? =
    when (uiMode and Configuration.UI_MODE_NIGHT_MASK) {
        Configuration.UI_MODE_NIGHT_YES -> ColorTheme.DARK
        Configuration.UI_MODE_NIGHT_NO -> ColorTheme.LIGHT
        else -> null
    }

fun ColorTheme.surfaces(): WarrenSurfaces =
    when (this) {
        ColorTheme.DARK -> WarrenSurfaces.Dark
        ColorTheme.LIGHT -> WarrenSurfaces.Light
    }
