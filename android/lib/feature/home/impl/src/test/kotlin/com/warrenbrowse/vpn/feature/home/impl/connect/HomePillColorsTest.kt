package com.warrenbrowse.vpn.feature.home.impl.connect

import com.warrenbrowse.vpn.lib.ui.theme.color.WarrenSurfaces
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue
import org.junit.jupiter.api.Test

/**
 * The status pill carries the phase in its fill, written in the home palette of the resolved theme
 * (desktop `HomeStatus` pillColors), and its eye is open while the user is visible.
 */
class HomePillColorsTest {

    private val dark = WarrenSurfaces.Dark
    private val light = WarrenSurfaces.Light

    @Test
    fun `the exposed pill is the salmon of the mockup`() {
        assertEquals(
            HomePillColors(dark.homePillExposed, dark.homePillExposedText),
            ConnectionPhase.Exposed.pillColors(dark),
        )
    }

    @Test
    fun `the protected pill is the green of the mockup`() {
        assertEquals(
            HomePillColors(light.homePillProtected, light.homePillProtectedText),
            ConnectionPhase.Protected.pillColors(light),
        )
    }

    @Test
    fun `an interrupted tunnel is pending like one coming up, since nothing flows in either`() {
        assertEquals(
            HomePillColors(dark.homePillPending, dark.homePillPendingText),
            ConnectionPhase.Connecting.pillColors(dark),
        )
        assertEquals(
            ConnectionPhase.Connecting.pillColors(dark),
            ConnectionPhase.Interrupted.pillColors(dark),
        )
    }

    @Test
    fun `the kill switch pill stays neutral, no hue being its signal`() {
        assertEquals(
            HomePillColors(light.homePillBlocked, light.homePillBlockedText),
            ConnectionPhase.Blocked.pillColors(light),
        )
    }

    @Test
    fun `the eye is open while the user is visible and shut once hidden`() {
        assertTrue(ConnectionPhase.Exposed.isEyeOpen())
        assertTrue(ConnectionPhase.Connecting.isEyeOpen())
        assertFalse(ConnectionPhase.Protected.isEyeOpen())
        assertFalse(ConnectionPhase.Interrupted.isEyeOpen())
        assertFalse(ConnectionPhase.Blocked.isEyeOpen())
    }
}
