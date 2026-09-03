package com.warrenbrowse.vpn.feature.home.impl.connect

import androidx.compose.ui.graphics.colorspace.ColorSpaces
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.ImageBitmapConfig
import androidx.compose.ui.graphics.colorspace.ColorSpace
import java.util.concurrent.CountDownLatch
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertSame
import kotlin.test.assertTrue
import org.junit.jupiter.api.Test

/**
 * The decode cache behind the scenery painters. The behaviours pinned here
 * are what turns three main-thread decodes per home frame into none: one
 * decode per master, a warm-up that a later frame finds, and a frame that
 * waits for a decode already running instead of starting its own.
 */
class SceneryBitmapCacheTest {

    /** A stand-in bitmap: the cache never reads pixels, only identity matters. */
    private class FakeBitmap(val id: Int) : ImageBitmap {
        override val width = 1
        override val height = 1
        override val config = ImageBitmapConfig.Argb8888
        override val colorSpace: ColorSpace = ColorSpaces.Srgb
        override val hasAlpha = false

        override fun prepareToDraw() = Unit

        override fun readPixels(
            buffer: IntArray,
            startX: Int,
            startY: Int,
            width: Int,
            height: Int,
            bufferOffset: Int,
            stride: Int,
        ) = Unit
    }

    @Test
    fun `a master is decoded once and served from the cache after`() {
        val decodes = AtomicInteger()
        val cache = SceneryBitmapCache(decode = { id -> decodes.incrementAndGet(); FakeBitmap(id) })

        val first = cache.get(7)
        val second = cache.get(7)

        assertSame(first, second, "the second read must be the cached bitmap")
        assertEquals(1, decodes.get(), "one decode per master")
    }

    @Test
    fun `a warmed master is found by the frame that asks for it`() {
        val decodes = AtomicInteger()
        val cache = SceneryBitmapCache(decode = { id -> decodes.incrementAndGet(); FakeBitmap(id) })
        assertFalse(cache.isWarm(3))

        cache.warm(3)

        assertTrue(cache.isWarm(3))
        cache.get(3)
        assertEquals(1, decodes.get(), "the frame must not decode what the warm-up decoded")
    }

    @Test
    fun `a frame asking for a master being decoded waits for that decode`() {
        val decodes = AtomicInteger()
        val decodeStarted = CountDownLatch(1)
        val release = CountDownLatch(1)
        val cache =
            SceneryBitmapCache(
                decode = { id ->
                    decodes.incrementAndGet()
                    decodeStarted.countDown()
                    release.await()
                    FakeBitmap(id)
                }
            )
        val executor = Executors.newFixedThreadPool(2)
        try {
            val warmUp = executor.submit<ImageBitmap> { cache.get(5) }
            decodeStarted.await()
            val frame = executor.submit<ImageBitmap> { cache.get(5) }
            // Give the frame's thread time to reach the cache while the decode
            // is still blocked; a second decode would have bumped the counter.
            Thread.sleep(100)
            assertEquals(1, decodes.get(), "the frame must wait, not decode again")

            release.countDown()

            assertSame(warmUp.get(), frame.get(), "both callers receive the one decoded bitmap")
            assertEquals(1, decodes.get())
        } finally {
            executor.shutdownNow()
        }
    }

    @Test
    fun `a frame asking for another master is not held behind a decode in flight`() {
        val release = CountDownLatch(1)
        val decodeStarted = CountDownLatch(1)
        val cache =
            SceneryBitmapCache(
                decode = { id ->
                    if (id == 5) {
                        decodeStarted.countDown()
                        release.await()
                    }
                    FakeBitmap(id)
                }
            )
        val executor = Executors.newFixedThreadPool(2)
        try {
            val slow = executor.submit<ImageBitmap> { cache.get(5) }
            decodeStarted.await()

            val other = executor.submit<ImageBitmap> { cache.get(6) }

            assertEquals(6, (other.get(2, TimeUnit.SECONDS) as FakeBitmap).id, "served while 5 is still decoding")
            release.countDown()
            slow.get(2, TimeUnit.SECONDS)
        } finally {
            executor.shutdownNow()
        }
    }

    @Test
    fun `the least recently drawn master is evicted past the capacity`() {
        val cache = SceneryBitmapCache(decode = { id -> FakeBitmap(id) }, capacity = 2)

        cache.get(1)
        cache.get(2)
        cache.get(1)
        cache.get(3)

        assertTrue(cache.isWarm(1), "drawn again, so kept")
        assertFalse(cache.isWarm(2), "the oldest of the three goes")
        assertTrue(cache.isWarm(3))
    }

    @Test
    fun `clearing drops every master`() {
        val cache = SceneryBitmapCache(decode = { id -> FakeBitmap(id) })
        cache.warm(1)
        cache.warm(2)

        cache.clear()

        assertFalse(cache.isWarm(1))
        assertFalse(cache.isWarm(2))
    }
}
