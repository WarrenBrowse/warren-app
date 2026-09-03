package com.warrenbrowse.vpn.lib.repository

/**
 * What this installation gave up when a higher-priority product environment
 * appeared on the device, and whether the user has since asked for this build
 * back.
 *
 * Neither mobile OS lets one app observe another app's VPN state, and both
 * already enforce that a single VPN is active at a time, so the rule here is
 * presence rather than state and it acts on the transition:
 * [higherEnvironmentSeen] holds the presence at the last observation and only a
 * change of it does anything. Without that memory a manual re-enable would be
 * undone at the next app start, every start, for as long as the other install
 * stays on the device.
 *
 * [restoreAutoConnect] and [restoreBlockingPolicy] are the two settings the
 * stand-down takes away, recorded before they are cleared so the manual
 * re-enable can put back exactly what the user had.
 */
data class WarrenEnvStandDown(
    val higherEnvironmentSeen: Boolean = false,
    val reEnabled: Boolean = false,
    val restoreAutoConnect: Boolean = false,
    val restoreBlockingPolicy: Boolean = false,
)

/**
 * Durable home of [WarrenEnvStandDown]. It has to survive a process restart and
 * an app update: a record that resets makes every start look like a first
 * detection, which is how a re-enable gets undone behind the user's back.
 */
interface WarrenEnvStandDownStore {
    fun readEnvStandDown(): WarrenEnvStandDown

    fun writeEnvStandDown(record: WarrenEnvStandDown)
}
