package com.warrenbrowse.vpn.lib.common.util

import kotlin.coroutines.cancellation.CancellationException
import kotlinx.coroutines.test.runTest
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

class CancellableResultTest {
    @Test
    fun `a value comes back as a success`() = runTest {
        assertEquals(Result.success(7), runCatchingCancellable { 7 })
    }

    @Test
    fun `a failure comes back as a failure`() = runTest {
        val result = runCatchingCancellable<Int> { error("refused") }
        assertTrue(result.exceptionOrNull() is IllegalStateException)
    }

    @Test
    fun `a cancellation is rethrown, never turned into a failure`() = runTest {
        var escaped: Throwable? = null
        try {
            runCatchingCancellable<Int> { throw CancellationException("left the screen") }
        } catch (e: CancellationException) {
            escaped = e
        }
        assertTrue(escaped is CancellationException)
    }
}
