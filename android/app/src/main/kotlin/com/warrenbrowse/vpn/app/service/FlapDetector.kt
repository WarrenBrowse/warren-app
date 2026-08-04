package com.warrenbrowse.vpn.app.service

/**
 * Sliding-window detector for a flapping tunnel: too many unexpected drops in
 * a short window. Mirrors the desktop state machine's flap guard, which parks
 * the tunnel in an error state once reconnects churn faster than the network
 * can settle, instead of spinning the uncancelable retry loop forever.
 *
 * On Android the only uncancelable loop is the lockdown reconnect in
 * [WarrenQuinnAdapter]: a drop under lockdown re-establishes the blackhole
 * interface and schedules another connect, which can drop again. Feeding each
 * such drop into [recordDrop] lets the adapter stop hammering once it is
 * clearly flapping and surface a [WarrenTunnelState.Blocking] flap state so
 * the user can act (the kill switch stays engaged the whole time).
 *
 * Pure and clock-injected (callers pass a monotonic timestamp) so the decision
 * is unit-testable without a device.
 */
class FlapDetector(
    private val threshold: Int = DEFAULT_THRESHOLD,
    private val windowMillis: Long = DEFAULT_WINDOW_MILLIS,
) {
    private val drops = ArrayDeque<Long>()

    /**
     * Record a drop at [nowMillis] (a monotonic timestamp) and report whether
     * the tunnel is now flapping: at least [threshold] drops fall within the
     * trailing [windowMillis] window. The window is inclusive of its boundary.
     */
    fun recordDrop(nowMillis: Long): Boolean {
        val cutoff = nowMillis - windowMillis
        while (drops.isNotEmpty() && drops.first() < cutoff) {
            drops.removeFirst()
        }
        drops.addLast(nowMillis)
        return drops.size >= threshold
    }

    /** Forget all recorded drops (call after a real reconnect succeeds). */
    fun reset() {
        drops.clear()
    }

    companion object {
        /**
         * Four drops trips the guard. At the 15 s lockdown-reconnect backoff
         * that is ~45 s of solid failure: long enough to rule out a one-off,
         * short enough to stop hammering a network that is clearly down.
         */
        const val DEFAULT_THRESHOLD = 4

        /** Drops older than this many ms no longer count toward flapping. */
        const val DEFAULT_WINDOW_MILLIS = 90_000L
    }
}
