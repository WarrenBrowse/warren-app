package com.warrenbrowse.vpn.feature.home.impl.connect

/** How the home keeps the status bar glyphs readable over what lies under them. */
internal data class HomeStatusBar(
    /** Dark glyphs, for the pale sky of every landscape and the light theme's page. */
    val darkGlyphs: Boolean,
    /** Darken the top of the hero under the status bar, so light glyphs read over the sky. */
    val skyScrim: Boolean,
)

/**
 * Stacked, the status bar sits on the sky alone, pale in every landscape, so its glyphs are dark
 * in both themes. Split ([split]), its right half sits on the page: the light theme's page carries
 * dark glyphs too, the dark theme's does not, so there the glyphs turn light and the sky under the
 * left half is darkened for them.
 */
internal fun homeStatusBar(split: Boolean, isDark: Boolean): HomeStatusBar {
    val lightOnPage = split && isDark
    return HomeStatusBar(darkGlyphs = !lightOnPage, skyScrim = lightOnPage)
}
