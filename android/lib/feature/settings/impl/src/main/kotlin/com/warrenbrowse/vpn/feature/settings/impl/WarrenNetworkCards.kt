package com.warrenbrowse.vpn.feature.settings.impl

import com.warrenbrowse.vpn.lib.model.ExitStats
import com.warrenbrowse.vpn.lib.model.WarrenNetworkStats
import com.warrenbrowse.vpn.lib.repository.WarrenRelaySummary

/**
 * One exit card of the Warren network screen: the snapshot's figures under the place the signed
 * relay list gives the exit. The snapshot is unsigned display data, so its own country and city are
 * only a fallback for an exit the relay list does not carry.
 */
data class NetworkExitCard(val exit: ExitStats, val country: String, val city: String)

/** The exit cards, the ones serving first, then by place. */
fun networkExitCards(
    stats: WarrenNetworkStats,
    relays: List<WarrenRelaySummary>,
): List<NetworkExitCard> {
    val places = relays.associateBy { it.exitId.lowercase() }
    return stats.exits
        .map { exit ->
            val relay = places[exit.exitId]
            NetworkExitCard(
                exit = exit,
                country = relay?.country ?: exit.country,
                city = relay?.city ?: exit.city,
            )
        }
        .sortedWith(
            compareBy<NetworkExitCard> { !it.exit.online }
                .thenBy { it.country }
                .thenBy { it.city }
                .thenBy { it.exit.exitId }
        )
}
