package com.warrenbrowse.vpn.app.service

import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

class FlapDetectorTest {

    @Test
    fun `a single drop is not flapping`() {
        val detector = FlapDetector(threshold = 4, windowMillis = 90_000L)
        assertFalse(detector.recordDrop(0L))
    }

    @Test
    fun `reaching the threshold within the window reports flapping`() {
        val detector = FlapDetector(threshold = 4, windowMillis = 90_000L)
        // Four drops spaced like the 15 s lockdown-reconnect backoff: well
        // inside the 90 s window, so the fourth trips the detector.
        assertFalse(detector.recordDrop(0L))
        assertFalse(detector.recordDrop(15_000L))
        assertFalse(detector.recordDrop(30_000L))
        assertTrue(detector.recordDrop(45_000L))
    }

    @Test
    fun `drops older than the window age out and do not trip`() {
        val detector = FlapDetector(threshold = 4, windowMillis = 90_000L)
        // Slow failures: each is more than a full window apart, so only the
        // newest one is ever in scope and the count never reaches threshold.
        assertFalse(detector.recordDrop(0L))
        assertFalse(detector.recordDrop(100_000L))
        assertFalse(detector.recordDrop(200_000L))
        assertFalse(detector.recordDrop(300_000L))
    }

    @Test
    fun `a drop exactly on the window boundary still counts`() {
        val detector = FlapDetector(threshold = 2, windowMillis = 90_000L)
        assertFalse(detector.recordDrop(0L))
        // The first drop is exactly windowMillis old: the window is inclusive
        // so it is still in scope and the second drop trips the detector.
        assertTrue(detector.recordDrop(90_000L))
    }

    @Test
    fun `reset clears accumulated drops so the loop can start fresh`() {
        val detector = FlapDetector(threshold = 2, windowMillis = 90_000L)
        assertFalse(detector.recordDrop(0L))
        detector.reset()
        // After a real reconnect (reset), a single later drop must not be
        // treated as the second of a flapping pair.
        assertFalse(detector.recordDrop(1_000L))
    }
}
