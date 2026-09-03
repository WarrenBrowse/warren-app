package com.warrenbrowse.vpn.feature.home.impl.connect

import androidx.compose.ui.graphics.BlendMode
import com.warrenbrowse.vpn.lib.ui.theme.tokens.DesignTokens
import kotlin.test.assertEquals
import org.junit.jupiter.api.Test

/**
 * The scenery backdrop's numbers against the desktop `CountryBackdrop`
 * (through the generated tokens): the connecting blur, dim and zoom, the phase
 * wash and every timing. The three clients tell the same visual story, so a
 * value moved on one side fails here.
 */
class SceneryParityTest {

    @Test
    fun `the connecting landscape blurs, dims and zooms as the desktop does`() {
        assertEquals(DesignTokens.Scenery.BlurRadius, LANDSCAPE_BLUR_RADIUS)
        assertEquals(DesignTokens.Scenery.ConnectingZoom, CONNECTING_ZOOM)
        // brightness(0.92) is a black overlay at 8 % over opaque art.
        assertEquals(1f - DesignTokens.Scenery.ConnectingBrightness, CONNECTING_DIM, 1e-6f)
    }

    @Test
    fun `the phase wash is the desktop soft-light wash`() {
        assertEquals(DesignTokens.Scenery.WashAlpha, WASH_ALPHA)
        assertEquals(DesignTokens.Scenery.WashTopStop, WASH_TOP_STOP)
        assertEquals(DesignTokens.Scenery.WashBottomStop, WASH_BOTTOM_STOP)
        assertEquals("soft-light", DesignTokens.Scenery.WashBlend)
        assertEquals(BlendMode.Softlight, WASH_BLEND)
    }

    @Test
    fun `every scenery timing is the desktop timing`() {
        assertEquals(DesignTokens.Scenery.BlurTransition, BLUR_MILLIS)
        assertEquals(DesignTokens.Scenery.ZoomTransition, ZOOM_MILLIS)
        assertEquals(DesignTokens.Scenery.Crossfade, CROSSFADE_MILLIS)
        assertEquals(DesignTokens.Scenery.WashTransition, CROSSFADE_MILLIS)
        assertEquals(DesignTokens.Scenery.BulaTransition, BULA_MILLIS)
        assertEquals(DesignTokens.Scenery.BulaHideDrop, BULA_HIDE_DROP)
    }
}
