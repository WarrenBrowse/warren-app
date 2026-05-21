package com.warrenbrowse.vpn.lib.usecase

import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.filterNotNull
import com.warrenbrowse.vpn.lib.common.util.entryBlocked
import com.warrenbrowse.vpn.lib.common.util.isMultihopEnabled
import com.warrenbrowse.vpn.lib.common.util.relaylist.findByGeoLocationId
import com.warrenbrowse.vpn.lib.common.util.wireguardConstraints
import com.warrenbrowse.vpn.lib.model.Constraint
import com.warrenbrowse.vpn.lib.model.CustomListId
import com.warrenbrowse.vpn.lib.model.GeoLocationId
import com.warrenbrowse.vpn.lib.model.HopSelection
import com.warrenbrowse.vpn.lib.model.RelayItem
import com.warrenbrowse.vpn.lib.model.RelayItemId
import com.warrenbrowse.vpn.lib.repository.RelayListRepository
import com.warrenbrowse.vpn.lib.repository.SettingsRepository
import com.warrenbrowse.vpn.lib.usecase.customlists.CustomListsRelayItemUseCase

class HopSelectionUseCase(
    private val customListRelayItemUseCase: CustomListsRelayItemUseCase,
    private val relayListRepository: RelayListRepository,
    private val settingsRepository: SettingsRepository,
) {
    operator fun invoke(): Flow<HopSelection> =
        combine(
            customListRelayItemUseCase(),
            relayListRepository.relayList,
            settingsRepository.settingsUpdates.filterNotNull(),
            relayListRepository.selectedLocation,
        ) { customLists, relayList, settings, selectedExitLocation ->
            if (settings.isMultihopEnabled()) {
                val entry =
                    if (settings.entryBlocked()) {
                        Constraint.Any
                    } else {
                        settings
                            .wireguardConstraints()
                            .entryLocation
                            .toRelayItemConstraint(customLists, relayList)
                    }
                HopSelection.Multi(
                    entry,
                    selectedExitLocation.toRelayItemConstraint(customLists, relayList),
                )
            } else {
                HopSelection.Single(
                    selectedExitLocation.toRelayItemConstraint(customLists, relayList)
                )
            }
        }

    private fun Constraint<RelayItemId>.toRelayItemConstraint(
        customLists: List<RelayItem.CustomList>,
        relayList: List<RelayItem.Location.Country>,
    ): Constraint<RelayItem>? =
        if (this is Constraint.Only) {
            when (val id = this.value) {
                is CustomListId -> customLists.firstOrNull { it.id == id }
                is GeoLocationId -> relayList.findByGeoLocationId(id)
            }?.let(Constraint<RelayItem>::Only)
        } else {
            Constraint.Any
        }
}
