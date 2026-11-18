package com.warrenbrowse.vpn.lib.common.util

import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runTest
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

@OptIn(ExperimentalCoroutinesApi::class)
class ProcessScopeTest {
    @Test
    fun `a failing job is reported and leaves its siblings running`() = runTest {
        val reported = mutableListOf<Throwable>()
        val scope = processScope(StandardTestDispatcher(testScheduler)) { reported += it }
        val sibling = CompletableDeferred<Unit>()

        scope.launch { error("one singleton's bug") }
        val survivor = scope.launch { sibling.await() }
        advanceUntilIdle()
        sibling.complete(Unit)
        advanceUntilIdle()

        assertEquals(listOf("one singleton's bug"), reported.map { it.message })
        assertTrue(survivor.isCompleted && !survivor.isCancelled)
    }
}
