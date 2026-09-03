package com.warrenbrowse.vpn.app.perf

import kotlin.test.assertEquals
import kotlin.test.assertNull
import org.junit.jupiter.api.Test

/**
 * The folding of JankStats frames into logcat lines: one summary per second
 * at most, only for seconds that dropped frames, so a device that misses
 * most deadlines does not drown logcat and a smooth session logs nothing.
 */
class JankLoggerTest {

    private val second = 1_000_000_000L
    private val ms = 1_000_000L

    @Test
    fun `a window with janky frames closes with one summary line`() {
        val logger = JankLogger(windowNanos = second)

        assertNull(logger.onFrame(frameEndNanos = 0, durationNanos = 8 * ms, isJank = false))
        assertNull(logger.onFrame(frameEndNanos = 300 * ms, durationNanos = 40 * ms, isJank = true))
        assertNull(logger.onFrame(frameEndNanos = 600 * ms, durationNanos = 25 * ms, isJank = true))

        val line = logger.onFrame(frameEndNanos = second + 10 * ms, durationNanos = 8 * ms, isJank = false)

        assertEquals("2 of 3 frames missed their deadline in the last second (worst 40 ms)", line)
    }

    @Test
    fun `a window where every frame made its deadline logs nothing`() {
        val logger = JankLogger(windowNanos = second)
        logger.onFrame(0, 8 * ms, isJank = false)
        logger.onFrame(500 * ms, 9 * ms, isJank = false)

        assertNull(logger.onFrame(second + 1, 8 * ms, isJank = false))
    }

    @Test
    fun `the frame that opens a window is counted in the new window, not the closed one`() {
        val logger = JankLogger(windowNanos = second)
        logger.onFrame(0, 8 * ms, isJank = false)

        // Closes the first window (clean) and opens the second with a janky frame.
        assertNull(logger.onFrame(second + 1, 50 * ms, isJank = true))
        val line = logger.onFrame(2 * second + 2, 8 * ms, isJank = false)

        assertEquals("1 of 1 frames missed their deadline in the last second (worst 50 ms)", line)
    }
}
