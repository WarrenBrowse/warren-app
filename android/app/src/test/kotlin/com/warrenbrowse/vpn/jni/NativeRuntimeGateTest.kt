package com.warrenbrowse.vpn.jni

import java.util.concurrent.CountDownLatch
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue
import org.junit.jupiter.api.Test

/**
 * The gate that moves the native init off the main thread: a caller that
 * needs the runtime proceeds only once the init has run, the init runs once
 * however often the application asks, and an init that throws still lets the
 * callers through.
 */
class NativeRuntimeGateTest {

    @Test
    fun `a caller waits for the init to finish before proceeding`() {
        val release = CountDownLatch(1)
        val initDone = AtomicInteger()
        val gate = NativeRuntimeGate {
            release.await()
            initDone.incrementAndGet()
        }
        gate.start()
        assertFalse(gate.isReady, "the init is still blocked")

        val executor = Executors.newSingleThreadExecutor()
        try {
            // What the caller sees the moment the gate lets it through.
            val initsSeenByCaller =
                executor.submit<Int> {
                    gate.awaitReadyBlocking()
                    initDone.get()
                }

            release.countDown()

            assertEquals(
                1,
                initsSeenByCaller.get(5, TimeUnit.SECONDS),
                "the caller must be let through only once the init has run",
            )
            assertTrue(gate.isReady)
        } finally {
            executor.shutdownNow()
        }
    }

    @Test
    fun `the init runs once however many times the gate is started`() {
        val runs = AtomicInteger()
        val gate = NativeRuntimeGate { runs.incrementAndGet() }

        gate.start()
        gate.start()
        gate.awaitReadyBlocking()

        assertEquals(1, runs.get())
    }

    @Test
    fun `an init that throws still releases the callers`() {
        // The Rust side answers every call fail-safe without a runtime; a
        // caller stranded forever would be strictly worse than that answer.
        val gate = NativeRuntimeGate { throw IllegalStateException("no log dir") }
        gate.start()

        val executor = Executors.newSingleThreadExecutor()
        try {
            executor.submit { gate.awaitReadyBlocking() }.get(5, TimeUnit.SECONDS)
        } finally {
            executor.shutdownNow()
        }
        assertTrue(gate.isReady)
    }
}
