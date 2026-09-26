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
    val host = endpoint.hostLiteral()
    val relay = relays.firstOrNull { host != null && it.endpoint.substringBeforeLast(':') == host }
    val exit = relay?.let { snapshot?.exit(it.exitId) }
    return if (snapshot != null && exit != null) ConnectedExitLoad(exit, snapshot) else null
}
