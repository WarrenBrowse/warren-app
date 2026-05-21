package com.warrenbrowse.vpn.lib.usecase

import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.filterNotNull
import com.warrenbrowse.vpn.lib.model.RelayItemSelection
import com.warrenbrowse.vpn.lib.repository.RelayListRepository
import com.warrenbrowse.vpn.lib.repository.WireguardConstraintsRepository

class SelectedLocationUseCase(
    private val relayListRepository: RelayListRepository,
    private val wireguardConstraintsRepository: WireguardConstraintsRepository,
) {
    operator fun invoke() =
        combine(
            relayListRepository.selectedLocation.filterNotNull(),
            wireguardConstraintsRepository.wireguardConstraints.filterNotNull(),
        ) { selectedLocation, wireguardConstraints ->
            if (wireguardConstraints.isMultihopEnabled) {
                RelayItemSelection.Multiple(
                    entryLocation = wireguardConstraints.entryLocation,
                    exitLocation = selectedLocation,
                )
            } else {
                RelayItemSelection.Single(selectedLocation)
            }
        }
}
