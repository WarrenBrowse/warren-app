package com.warrenbrowse.vpn.feature.home.impl.connect

import com.warrenbrowse.vpn.lib.ui.resource.R
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue
import org.junit.jupiter.api.Test

/**
 * The backdrop each connection phase paints. This is the Android arm of the
 * desktop `connection-scenery.spec.ts` contract: the three clients tell the
 * same visual story, so the mapping is pinned on every platform.
 */
class SceneryStateTest {

    @Test
    fun `exposed shows the watched plain sharp with the rabbit outside`() {
        // The exit country is deliberately ignored: no tunnel carries traffic
        // there, so showing its cityscape would claim a protection that is not
        // in place.
        val scenery = resolveScenery(ConnectionPhase.Exposed, "germany")

        assertEquals(R.drawable.scenery_plaine, scenery.landscape)
        assertTrue(scenery.showBula)
        assertFalse(scenery.blurred)
    }

    @Test
    fun `blocked shows the watched plain blurred with the rabbit tucked in`() {
        val scenery = resolveScenery(ConnectionPhase.Blocked, "germany")

        assertEquals(R.drawable.scenery_plaine, scenery.landscape)
        assertFalse(scenery.showBula)
        assertTrue(scenery.blurred)
    }

    @Test
    fun `the exit cityscape backs the tunnelled phases, case and code insensitively`() {
        assertEquals(
            R.drawable.scenery_germany,
            resolveScenery(ConnectionPhase.Protected, " Germany ").landscape,
        )
        assertEquals(
            R.drawable.scenery_finland,
            resolveScenery(ConnectionPhase.Protected, "FI").landscape,
        )
        assertEquals(
            R.drawable.scenery_netherlands,
            resolveScenery(ConnectionPhase.Connecting, "netherlands").landscape,
        )
        assertEquals(
            R.drawable.scenery_singapore,
            resolveScenery(ConnectionPhase.Interrupted, "sg").landscape,
        )
    }

    @Test
    fun `an exit with no bespoke art falls back to the plain`() {
        assertEquals(
            R.drawable.scenery_plaine,
            resolveScenery(ConnectionPhase.Protected, "France").landscape,
        )
        assertEquals(
            R.drawable.scenery_plaine,
            resolveScenery(ConnectionPhase.Protected, null).landscape,
        )
    }

    @Test
    fun `the rabbit is only outside before the tunnel is up`() {
        assertTrue(resolveScenery(ConnectionPhase.Connecting, "fi").showBula)
        assertFalse(resolveScenery(ConnectionPhase.Protected, "fi").showBula)
        // Nominally-up tunnel with nothing flowing: fail-closed, so the rabbit
        // stays in the burrow and only the landscape reads "not settled".
        assertFalse(resolveScenery(ConnectionPhase.Interrupted, "fi").showBula)
        assertTrue(resolveScenery(ConnectionPhase.Interrupted, "fi").blurred)
    }
}
