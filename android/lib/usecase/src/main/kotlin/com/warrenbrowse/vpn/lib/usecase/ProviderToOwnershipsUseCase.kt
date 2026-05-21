package com.warrenbrowse.vpn.lib.usecase

import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.map
import com.warrenbrowse.vpn.lib.model.Ownership
import com.warrenbrowse.vpn.lib.model.ProviderId
import com.warrenbrowse.vpn.lib.model.RelayItem
import com.warrenbrowse.vpn.lib.repository.RelayListRepository

class ProviderToOwnershipsUseCase(private val relayListRepository: RelayListRepository) {
    operator fun invoke(): Flow<Map<ProviderId, Set<Ownership>>> =
        relayListRepository.relayList.map { relayList ->
            relayList
                .flatMap(RelayItem.Location.Country::cities)
                .flatMap(RelayItem.Location.City::relays)
                .groupBy({ it.provider }, { it.ownership })
                .mapValues { (_, ownerships) -> ownerships.toSet() }
        }
}
