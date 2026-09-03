package com.warrenbrowse.vpn.app.perf

/**
 * Turns the stream of frame timings JankStats reports into one logcat line
 * per second at most, and only for seconds that had a janky frame.
 *
 * On-device only: the line goes to logcat and nowhere else (Warren keeps no
 * telemetry), so a tester reading `adb logcat -s WarrenJank` sees where the
 * frames went during a session, and a release build carries the same
 * accounting a debug build does. Per-frame lines would drown logcat on a
 * device that drops most frames (the emulator drops nearly all of them
 * during the connecting animation), so the frames are folded into a summary.
 *
 * Pure: [onFrame] returns the line to log, or `null`, so the folding is
 * testable without a window.
 */
class JankLogger(private val windowNanos: Long = DEFAULT_WINDOW_NANOS) {
    private var windowStart = Long.MIN_VALUE
    private var frames = 0
    private var janky = 0
    private var worstNanos = 0L

    /**
     * Account one frame that ENDED at [frameEndNanos] (a monotonic clock) with
     * [durationNanos] of UI thread time, [isJank] per the framework's deadline.
     * Returns the summary line closing the previous window when this frame
     * opens a new one, `null` otherwise.
     */
    fun onFrame(frameEndNanos: Long, durationNanos: Long, isJank: Boolean): String? {
        var closing: String? = null
        if (windowStart == Long.MIN_VALUE) {
            windowStart = frameEndNanos
        } else if (frameEndNanos - windowStart >= windowNanos) {
            closing = summary()
            windowStart = frameEndNanos
            frames = 0
            janky = 0
            worstNanos = 0L
        }
        frames++
        if (isJank) {
            janky++
            if (durationNanos > worstNanos) worstNanos = durationNanos
        }
        return closing
    }

    /** The line for the window just closed, or `null` when every frame of it made its deadline. */
    private fun summary(): String? {
        if (janky == 0) return null
        val worstMs = worstNanos / NANOS_PER_MILLI
        return "$janky of $frames frames missed their deadline in the last second (worst $worstMs ms)"
    }

    private companion object {
        const val DEFAULT_WINDOW_NANOS = 1_000_000_000L
        const val NANOS_PER_MILLI = 1_000_000L
    }
}
