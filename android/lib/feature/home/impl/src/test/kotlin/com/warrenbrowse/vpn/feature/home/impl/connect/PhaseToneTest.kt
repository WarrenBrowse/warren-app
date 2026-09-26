package com.warrenbrowse.vpn.feature.home.impl.connect

import kotlin.test.assertEquals
import org.junit.jupiter.api.Test

/**
 * Which colour token each phase fills and writes with, from scenery.json: the
 * same assignment desktop reads in `connection-phase.ts` and the browser
 * extension reads in its home model. The theme maps each token once.
 */
class PhaseToneTest {

    @Test
    fun `each phase fills with its saturated accent`() {
        assertEquals(SceneryTone.Red, ConnectionPhase.Exposed.accentTone())
        assertEquals(SceneryTone.Orange, ConnectionPhase.Connecting.accentTone())
        assertEquals(SceneryTone.Green, ConnectionPhase.Protected.accentTone())
        assertEquals(SceneryTone.Orange, ConnectionPhase.Interrupted.accentTone())
        assertEquals(SceneryTone.White, ConnectionPhase.Blocked.accentTone())
    }

    @Test
    fun `each phase writes its title with the lifted tint, blocked staying neutral`() {
        assertEquals(SceneryTone.RedText, ConnectionPhase.Exposed.titleTone())
        assertEquals(SceneryTone.OrangeText, ConnectionPhase.Connecting.titleTone())
        assertEquals(SceneryTone.GreenText, ConnectionPhase.Protected.titleTone())
        assertEquals(SceneryTone.OrangeText, ConnectionPhase.Interrupted.titleTone())
        assertEquals(SceneryTone.White, ConnectionPhase.Blocked.titleTone())
    }
}
