package com.warrenbrowse.vpn.feature.home.impl.connect

import kotlin.test.assertEquals
import org.junit.jupiter.api.Test

/**
 * The status bar glyphs must read over everything under them: the pale sky of every landscape,
 * and in split mode the page on the right half too.
 */
class HomeStatusBarTest {

    @Test
    fun `stacked, the glyphs are dark over the sky in both themes and nothing is scrimmed`() {
        assertEquals(HomeStatusBar(darkGlyphs = true, skyScrim = false), homeStatusBar(false, true))
        assertEquals(HomeStatusBar(darkGlyphs = true, skyScrim = false), homeStatusBar(false, false))
    }

    @Test
    fun `split in the light theme keeps dark glyphs, which the light panel and the sky both carry`() {
        assertEquals(HomeStatusBar(darkGlyphs = true, skyScrim = false), homeStatusBar(true, false))
    }

    @Test
    fun `split in the dark theme turns the glyphs light and darkens the sky under them`() {
        assertEquals(HomeStatusBar(darkGlyphs = false, skyScrim = true), homeStatusBar(true, true))
    }
}
