package com.warrenbrowse.vpn.lib.common.util

import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.CoroutineExceptionHandler
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob

/**
 * The dispatchers the app runs its work on, injected so a test swaps all three at once instead of
 * class by class.
 */
data class AppDispatchers(
    val io: CoroutineDispatcher,
    val default: CoroutineDispatcher,
    val main: CoroutineDispatcher,
) {
    companion object {
        val Standard = AppDispatchers(Dispatchers.IO, Dispatchers.Default, Dispatchers.Main)
    }
}

/**
 * The scope the process-wide singletons launch their work in. A job that throws is handed to
 * [onUncaught] and its siblings keep running: without a handler, one singleton's bug in a scope
 * like `MainScope()` ends the whole process, and the tunnel with it.
 */
fun processScope(main: CoroutineDispatcher, onUncaught: (Throwable) -> Unit): CoroutineScope =
    CoroutineScope(
        SupervisorJob() + main + CoroutineExceptionHandler { _, throwable -> onUncaught(throwable) }
    )
