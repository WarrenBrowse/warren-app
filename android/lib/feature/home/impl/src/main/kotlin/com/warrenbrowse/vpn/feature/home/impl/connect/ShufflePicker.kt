package com.warrenbrowse.vpn.feature.home.impl.connect

import com.warrenbrowse.vpn.lib.repository.ExitPin
import com.warrenbrowse.vpn.lib.repository.WarrenRelaySummary

/**
 * The exits a "surprise me" shuffle may land on.
 *
 * The exit currently in use is excluded, so the shuffle always changes
 * something: picking the location the user is already on reads as a broken
 * button. It comes back into the candidates only when it is the sole active
 * exit, where re-dialling it is the honest answer.
 */
internal fun shuffleCandidates(
    relays: List<WarrenRelaySummary>,
    currentExitId: String?,
): List<WarrenRelaySummary> {
    val active = relays.filter { it.active }
    return active.filter { it.exitId != currentExitId }.ifEmpty { active }
}

/**
 * The exit in use right now: the one the tunnel is dialling or running on when
 * there is a tunnel, otherwise the pinned one. A pin coarser than an exit
 * (a country, a city, or Automatic) names no single exit to exclude.
 */
internal fun currentExitId(
    relays: List<WarrenRelaySummary>,
    activeEndpointHost: String?,
    pin: ExitPin,
): String? {
    val live = activeEndpointHost?.let { host ->
        relays.firstOrNull { it.endpoint.substringBeforeLast(':') == host }?.exitId
    }
    return live ?: (pin as? ExitPin.Exit)?.exitId
}
