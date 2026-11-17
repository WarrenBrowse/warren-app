package com.warrenbrowse.vpn.lib.common.util

import kotlin.coroutines.cancellation.CancellationException

/**
 * [runCatching] for a block that suspends. The standard one also catches the [CancellationException]
 * that ends a cancelled coroutine, which then runs on as if its call had failed and reports a
 * failure nobody asked for. This one lets the cancellation through and catches everything else.
 */
@Suppress("TooGenericExceptionCaught")
suspend inline fun <T> runCatchingCancellable(block: suspend () -> T): Result<T> =
    try {
        Result.success(block())
    } catch (e: CancellationException) {
        throw e
    } catch (e: Throwable) {
        Result.failure(e)
    }
