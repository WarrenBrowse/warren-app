package com.warrenbrowse.vpn.feature.home.impl.connect

import com.warrenbrowse.vpn.lib.model.LoadLevel
import com.warrenbrowse.vpn.lib.ui.theme.color.WarrenSurfaces
import kotlin.test.assertEquals
import kotlin.test.assertNull
import org.junit.jupiter.api.Test

/**
 * The load ring on the exit row takes the home load colours (desktop `HomeStatus` load-ring
 * variables): its own green for a quiet exit, the pill colours as the load rises, on the load
 * track, in either theme.
 */
class HomeLoadRingPaletteTest {

    private val surfaces = WarrenSurfaces.Light
    private val palette = homeLoadRingPalette(surfaces)

    @Test
    fun `each band is painted in the home colour of its severity`() {
        assertEquals(surfaces.homeLoad, palette.colorOf(LoadLevel.LOW))
        assertEquals(surfaces.homeBadge, palette.colorOf(LoadLevel.MODERATE))
        assertEquals(surfaces.homePillPending, palette.colorOf(LoadLevel.HIGH))
        assertEquals(surfaces.homePillExposed, palette.colorOf(LoadLevel.SATURATED))
    }

    @Test
    fun `an unknown band keeps the neutral ring`() {
        assertNull(palette.colorOf(LoadLevel.UNKNOWN))
    }

    @Test
    fun `the ring runs on the home load track`() {
        assertEquals(surfaces.homeLoadTrack, palette.track)
    }
}
