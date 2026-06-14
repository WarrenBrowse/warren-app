package com.warrenbrowse.vpn.app.service

/**
 * What to do when an active tunnel session goes down. Pure decision so the
 * kill-switch / leak-prevention branching can be unit-tested without a live
 * `VpnService` (the side effects - establishing the blackhole interface,
 * scheduling a reconnect, releasing traffic - stay in [WarrenQuinnAdapter]).
 */
enum class KillSwitchAction {
    /** Release traffic to the physical network and surface a failure. */
    RELEASE,

    /** Keep the blackhole interface up and stay parked until the user acts. */
    PARK,

    /** Keep the blackhole interface up and schedule an automatic reconnect. */
    BLOCK_AND_RETRY,
}

object KillSwitchPolicy {
    /**
     * Decide the resting action for a dropped session.
     *
     * Invariants (the Mullvad fail-closed model):
     *  - A user-initiated teardown [RELEASE]s: the user asked to disconnect.
     *  - An unexpected single drop always [BLOCK_AND_RETRY]s, regardless of
     *    the kill-switch setting, so traffic never leaks while the tunnel
     *    recovers.
     *  - Once the tunnel is clearly [flapping] (repeated drops), the
     *    kill-switch setting decides the resting state: [PARK] (stay blocked)
     *    when lockdown is on, [RELEASE] when it is off so the user is not
     *    stranded offline.
     */
    fun decide(
        userInitiated: Boolean,
        flapping: Boolean,
        lockdownMode: Boolean,
    ): KillSwitchAction =
        when {
            userInitiated -> KillSwitchAction.RELEASE
            flapping && lockdownMode -> KillSwitchAction.PARK
            flapping -> KillSwitchAction.RELEASE
            else -> KillSwitchAction.BLOCK_AND_RETRY
        }
}
