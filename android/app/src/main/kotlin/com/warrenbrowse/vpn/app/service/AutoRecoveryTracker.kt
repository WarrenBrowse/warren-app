package com.warrenbrowse.vpn.app.service

/**
 * Kotlin-side attribution of automatic recoveries, mirroring the desktop
 * `warren_status::auto_recovery_step` semantics: a recovery is counted
 * only when a Connected arrives while a retry that ONLY automation
 * schedules is pending; user actions (connect, disconnect, manual
 * reconnect) never count and clear any pending attribution.
 *
 * This covers the recoveries the Rust redial engine cannot see: the
 * blackhole retry loop and the handover reconnect both tear the native
 * session down and dial a fresh one, so their success is a new initial
 * connect on the Rust side. In-session redials are counted by the engine
 * itself (`WarrenJni.getAutoRecoveryCount()`); the adapter sums both.
 */
class AutoRecoveryTracker {
    private var pending = false

    var count: Int = 0
        private set

    /** Automation scheduled a retry (drop reconnect, handover reconnect). */
    @Synchronized
    fun armAutomation() {
        pending = true
    }

    /** The user acted (teardown, manual reconnect): nothing that follows
     *  is an automatic recovery. */
    @Synchronized
    fun onUserAction() {
        pending = false
    }

    /** The tunnel reached Connected. Returns true when this landing
     *  completed a pending automatic retry (and was counted). */
    @Synchronized
    fun onConnected(): Boolean {
        if (!pending) return false
        pending = false
        count += 1
        return true
    }
}
