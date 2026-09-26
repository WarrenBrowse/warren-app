package com.warrenbrowse.vpn.feature.home.impl.connect

import com.warrenbrowse.vpn.lib.model.Endpoint
import com.warrenbrowse.vpn.lib.model.ExitStats
import com.warrenbrowse.vpn.lib.model.WarrenNetworkStats
import com.warrenbrowse.vpn.lib.repository.WarrenRelaySummary

/** The exit carrying the tunnel, as the network stats snapshot describes it. */
data class ConnectedExitLoad(val exit: ExitStats, val stats: WarrenNetworkStats)

/**
 * The load of the exit behind [endpoint]: the endpoint host names a relay of the signed catalogue,
 * whose `exit_id` keys the snapshot. Null when any link of that chain is missing.
 */
internal fun connectedExitLoad(
    endpoint: Endpoint,
    relays: List<WarrenRelaySummary>,
    snapshot: WarrenNetworkStats?,
): ConnectedExitLoad? {
    val host = endpoint.hostLiteral() ?: return null
    val relay = relays.firstOrNull { it.endpoint.substringBeforeLast(':') == host } ?: return null
    val exit = snapshot?.exit(relay.exitId) ?: return null
    return ConnectedExitLoad(exit, snapshot)
}
