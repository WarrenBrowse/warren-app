package com.warrenbrowse.vpn.lib.usecase

import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.filterNotNull
import kotlinx.coroutines.flow.map
import mullvad_daemon.relay_selector.exitConstraints
import com.warrenbrowse.vpn.lib.common.util.isDaitaAndNotDirectOnly
import com.warrenbrowse.vpn.lib.common.util.relaylist.filter
import com.warrenbrowse.vpn.lib.grpc.ManagementService
import com.warrenbrowse.vpn.lib.model.Constraint
import com.warrenbrowse.vpn.lib.model.DiscardedRelay
import com.warrenbrowse.vpn.lib.model.EntryConstraints
import com.warrenbrowse.vpn.lib.model.ExitConstraints
import com.warrenbrowse.vpn.lib.model.MultihopConstraints
import com.warrenbrowse.vpn.lib.model.MultihopRelayListType
import com.warrenbrowse.vpn.lib.model.RelayItem
import com.warrenbrowse.vpn.lib.model.RelayItemId
import com.warrenbrowse.vpn.lib.model.RelayListType
import com.warrenbrowse.vpn.lib.model.RelayPartitions
import com.warrenbrowse.vpn.lib.model.RelaySelectorPredicate
import com.warrenbrowse.vpn.lib.model.Settings
import com.warrenbrowse.vpn.lib.repository.RelayListRepository
import com.warrenbrowse.vpn.lib.repository.SettingsRepository

class FilteredRelayListUseCase(
    private val relayListRepository: RelayListRepository,
    private val settingsRepository: SettingsRepository,
    private val managementService: ManagementService,
) {
    operator fun invoke(relayListType: RelayListType) =
        combine(
            settingsRepository.settingsUpdates
                .filterNotNull()
                .map {
                    when (relayListType) {
                        is RelayListType.Multihop ->
                            when (relayListType.multihopRelayListType) {
                                MultihopRelayListType.ENTRY ->
                                    RelaySelectorPredicate.Entry(
                                        multihopConstraints =
                                            MultihopConstraints(
                                                entryConstraints =
                                                    it.toEntryConstraint(Constraint.Any),
                                                exitConstraints = it.toExitConstraint(),
                                            )
                                    )
                                MultihopRelayListType.EXIT ->
                                    RelaySelectorPredicate.Exit(
                                        multihopConstraints =
                                            MultihopConstraints(
                                                entryConstraints = it.toEntryConstraint(),
                                                exitConstraints =
                                                    it.toExitConstraint(Constraint.Any),
                                            )
                                    )
                            }
                        RelayListType.Single ->
                            if (it.isDaitaAndNotDirectOnly()) {
                                RelaySelectorPredicate.Autohop(it.toEntryConstraint(Constraint.Any))
                            } else {
                                RelaySelectorPredicate.SingleHop(
                                    it.toEntryConstraint(Constraint.Any)
                                )
                            }
                    }
                }
                .distinctUntilChanged()
                .map {
                    // We expect this to always work
                    managementService.partitionRelays(it).getOrNull()!!
                },
            relayListRepository.relayList,
        ) { partitions, relayList ->
            relayList.filter(partitions.relevantHostnames())
        }

    private fun RelayPartitions.relevantHostnames() =
        matches + discards.filter { it.shouldBeShown() }.map { it.hostname }

    private fun DiscardedRelay.shouldBeShown(): Boolean =
        with(why) {
            (conflictWithOtherHop or inactive) &&
                !location &&
                !providers &&
                !ownership &&
                !ipVersion &&
                !daita &&
                !obfuscation &&
                !port
        }

    private fun List<RelayItem.Location.Country>.filter(validHostnames: List<String>) = mapNotNull {
        it.filter(validHostnames)
    }
}

private fun Settings.toEntryConstraint(
    overrideExitLocation: Constraint<RelayItemId>? = null
): EntryConstraints =
    EntryConstraints(
        generalConstraints =
            ExitConstraints(
                location = overrideExitLocation ?: relaySettings.relayConstraints.location,
                providers = relaySettings.relayConstraints.providers,
                ownership = relaySettings.relayConstraints.ownership,
            ),
        obfuscation = Constraint.Only(obfuscationSettings),
        daitaSettings = Constraint.Only(tunnelOptions.daitaSettings),
        ipVersion = relaySettings.relayConstraints.wireguardConstraints.ipVersion,
    )

private fun Settings.toExitConstraint(
    overrideEntryLocation: Constraint<RelayItemId>? = null
): ExitConstraints =
    ExitConstraints(
        location = overrideEntryLocation ?: relaySettings.relayConstraints.location,
        providers = relaySettings.relayConstraints.providers,
        ownership = relaySettings.relayConstraints.ownership,
    )
