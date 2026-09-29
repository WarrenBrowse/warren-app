package com.warrenbrowse.vpn.feature.home.impl.connect

import com.warrenbrowse.vpn.lib.ui.theme.color.WarrenSurfaces
import kotlin.test.assertEquals
import org.junit.jupiter.api.Test

/**
 * The card is a surface of its own, light or dark with the theme, so it writes each phase in the
 * surface palette (desktop `getPhaseCardColors`) rather than in the scenery accents.
 */
class PhaseCardColorsTest {

    private val dark = WarrenSurfaces.Dark
    private val light = WarrenSurfaces.Light

    @Test
    fun `the exposed state is written in the salmon of the mockup`() {
        assertEquals(
            PhaseCardColors(title = dark.exposed, well = dark.exposedWell),
            ConnectionPhase.Exposed.cardColors(dark),
        )
    }

    @Test
    fun `the protected state is written in the green of the mockup`() {
        assertEquals(
            PhaseCardColors(title = light.protected, well = light.protectedWell),
            ConnectionPhase.Protected.cardColors(light),
        )
    }

    @Test
    fun `an interrupted tunnel is written like one coming up, since nothing flows in either`() {
        assertEquals(
            PhaseCardColors(title = dark.connecting, well = dark.connectingWell),
            ConnectionPhase.Connecting.cardColors(dark),
        )
        assertEquals(
            ConnectionPhase.Connecting.cardColors(dark),
            ConnectionPhase.Interrupted.cardColors(dark),
        )
    }

    @Test
    fun `the kill switch stays neutral, no hue being its signal`() {
        assertEquals(
            PhaseCardColors(title = dark.text, well = dark.button),
            ConnectionPhase.Blocked.cardColors(dark),
        )
    }
}
