package com.warrenbrowse.vpn.feature.home.impl.connect

import com.warrenbrowse.vpn.lib.ui.theme.tokens.DesignTokens
import kotlin.test.assertEquals
import org.junit.jupiter.api.Test

/**
 * The hero's numbers against the desktop `CountryBackdrop`: the timings and the blur come from the
 * generated home tokens, the rest are the desktop literals the hero carries as its own constants.
 * The three clients tell the same visual story, so a value moved on one side fails here.
 */
class SceneryParityTest {

    @Test
    fun `the connecting landscape blurs, dims and zooms as the desktop does`() {
        assertEquals(14f, DesignTokens.Home.Hero.BlurRadius.value)
        // brightness(0.92) is a black overlay at 8 % over opaque art.
        assertEquals(0.08f, CONNECTING_DIM, 1e-6f)
        assertEquals(1.08f, CONNECTING_ZOOM)
        assertEquals(900, BLUR_MILLIS)
        assertEquals(6000, ZOOM_MILLIS)
    }

    @Test
    fun `Bula slips 3 percent of the canvas down into the burrow`() {
        assertEquals(0.03f, BULA_HIDE_DROP)
    }
}
