package com.warrenbrowse.vpn.jni

import co.touchlab.kermit.Logger
import java.util.concurrent.atomic.AtomicBoolean
import kotlin.concurrent.thread
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.runBlocking

/**
 * Runs the native initialisation once on its own thread and holds every
 * caller that needs its result until it is done.
 *
 * `WarrenJni.initLogger` loads the 10 MB engine library, builds the tokio
 * runtime and opens the Rust log file: tens of milliseconds that used to sit
 * inside `Application.onCreate` on the main thread, ahead of the first frame.
 * Nothing on the startup path needs the runtime before its first network or
 * tunnel call, so the init moves to a background thread and the callers that
 * do need it wait here instead, off the main thread, for a wait that is
 * usually already over.
 *
 * A failed init releases the waiters too: the Rust side answers every call
 * without a runtime fail-safe (a fallback list, an unknown verdict, an
 * exception the caller already catches), which is what happened before when
 * the synchronous init threw.
 */
class NativeRuntimeGate(private val init: () -> Unit) {
    private val started = AtomicBoolean(false)
    private val ready = CompletableDeferred<Unit>()

    /** Start the init on a background thread; later calls are no-ops. */
    fun start() {
        if (!started.compareAndSet(false, true)) return
        thread(name = THREAD_NAME) {
            try {
                init()
            } catch (e: RuntimeException) {
                Logger.e(throwable = e) { "native runtime init failed" }
            } finally {
                ready.complete(Unit)
            }
        }
    }

    val isReady: Boolean
        get() = ready.isCompleted

    suspend fun awaitReady() = ready.await()

    /**
     * Block the calling thread until the init is done. For the non-suspending
     * JNI call sites, which already run on IO; never call it on the main
     * thread.
     */
    fun awaitReadyBlocking() {
        if (ready.isCompleted) return
        runBlocking { ready.await() }
    }

    private companion object {
        const val THREAD_NAME = "warren-native-init"
    }
}

/**
 * The process-wide gate in front of [WarrenJni]: started by the application,
 * awaited by every call that needs the engine runtime or the Rust log file.
 */
object WarrenNativeRuntime {
    @Volatile private var gate: NativeRuntimeGate? = null

    /** Start the native init for [filesDirectory] in the background, once. */
    fun start(filesDirectory: String) {
        val current = gate ?: NativeRuntimeGate { WarrenJni.initLogger(filesDirectory) }.also { gate = it }
        current.start()
    }

    /** Wait, off the main thread, until the native init has run. */
    fun awaitReadyBlocking() {
        gate?.awaitReadyBlocking() ?: Logger.w("native runtime awaited before the application started it")
    }
}
