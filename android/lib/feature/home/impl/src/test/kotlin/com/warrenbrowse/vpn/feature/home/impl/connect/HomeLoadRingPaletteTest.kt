package com.warrenbrowse.vpn.feature.home.impl.connect

import com.warrenbrowse.vpn.lib.model.LoadLevel
import com.warrenbrowse.vpn.lib.ui.theme.color.WarrenSurfaces
import kotlin.test.assertEquals
import kotlin.test.assertNull
import org.junit.jupiter.api.Test

/**
 * The load ring on the subtitle row takes the pill colours of the home palette (desktop
 * `HomeStatus` load-ring variables), so a quiet exit reads in the same green as the protected pill,
 * in either theme.
 */
class HomeLoadRingPaletteTest {

    private val surfaces = WarrenSurfaces.Light
    private val palette = homeLoadRingPalette(surfaces)

    @Test
    fun `each band is painted in the home colour of its severity`() {
        assertEquals(surfaces.homePillProtected, palette.colorOf(LoadLevel.LOW))
        assertEquals(surfaces.homeBadge, palette.colorOf(LoadLevel.MODERATE))
        assertEquals(surfaces.homePillPending, palette.colorOf(LoadLevel.HIGH))
        assertEquals(surfaces.homePillExposed, palette.colorOf(LoadLevel.SATURATED))
    }

    @Test
    fun `an unknown band keeps the neutral ring`() {
        assertNull(palette.colorOf(LoadLevel.UNKNOWN))
    }

    @Test
    fun `the track is the secondary text at 30 percent`() {
        assertEquals(surfaces.homeTextSecondary.copy(alpha = 0.3f), palette.track)
    }
}
