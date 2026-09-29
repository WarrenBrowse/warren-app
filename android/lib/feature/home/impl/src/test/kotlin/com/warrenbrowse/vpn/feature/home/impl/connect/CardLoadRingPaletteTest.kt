package com.warrenbrowse.vpn.feature.home.impl.connect

import com.warrenbrowse.vpn.lib.model.LoadLevel
import com.warrenbrowse.vpn.lib.ui.theme.color.WarrenSurfaces
import kotlin.test.assertEquals
import kotlin.test.assertNull
import org.junit.jupiter.api.Test

/**
 * The load ring on the hostname line takes the phase colours of the card (desktop Hostname), so a
 * quiet exit reads in the same green as a protected title, in either theme.
 */
class CardLoadRingPaletteTest {

    private val surfaces = WarrenSurfaces.Light
    private val palette = cardLoadRingPalette(surfaces)

    @Test
    fun `each band is painted in the card colour of its severity`() {
        assertEquals(surfaces.protected, palette.colorOf(LoadLevel.LOW))
        assertEquals(surfaces.pill, palette.colorOf(LoadLevel.MODERATE))
        assertEquals(surfaces.connecting, palette.colorOf(LoadLevel.HIGH))
        assertEquals(surfaces.exposed, palette.colorOf(LoadLevel.SATURATED))
    }

    @Test
    fun `an unknown band keeps the neutral ring`() {
        assertNull(palette.colorOf(LoadLevel.UNKNOWN))
    }

    @Test
    fun `the track is the muted text at 30 percent`() {
        assertEquals(surfaces.textMuted.copy(alpha = 0.3f), palette.track)
    }
}
