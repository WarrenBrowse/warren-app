package com.warrenbrowse.vpn.feature.home.impl.connect

import kotlin.test.assertEquals
import kotlin.test.assertNotSame
import kotlin.test.assertNull
import kotlin.test.assertSame
import org.junit.jupiter.api.Test

/**
 * The per-radius effect cache the scenery draw lambdas read on every frame:
 * a frame that keeps the blur it had must not allocate a new effect.
 */
class RenderEffectCacheTest {

    private class Effect(val radius: Float)

    @Test
    fun `frames with the same radius share one effect`() {
        var built = 0
        val cache = RenderEffectCache { radius -> built++; Effect(radius) }

        val first = cache.effect(12f)
        val second = cache.effect(12f)

        assertSame(first, second)
        assertEquals(1, built, "the second frame must reuse the effect, not rebuild it")
    }

    @Test
    fun `a new radius builds a new effect`() {
        val cache = RenderEffectCache { radius -> Effect(radius) }

        val first = cache.effect(12f)
        val second = cache.effect(13f)

        assertNotSame(first, second)
        assertEquals(13f, second?.radius)
    }

    @Test
    fun `a zero radius is no effect`() {
        // A zero-radius blur is invalid for the platform, and no blur is no
        // effect at all rather than a degenerate one.
        val cache = RenderEffectCache { radius -> Effect(radius) }

        assertNull(cache.effect(0f))
    }
}
