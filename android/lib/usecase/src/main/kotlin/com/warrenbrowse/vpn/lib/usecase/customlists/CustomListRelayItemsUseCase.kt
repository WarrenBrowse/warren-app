package com.warrenbrowse.vpn.lib.usecase.customlists

import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.mapNotNull
import com.warrenbrowse.vpn.lib.common.util.relaylist.getById
import com.warrenbrowse.vpn.lib.common.util.relaylist.getRelayItemsByCodes
import com.warrenbrowse.vpn.lib.model.CustomListId
import com.warrenbrowse.vpn.lib.model.RelayItem
import com.warrenbrowse.vpn.lib.repository.CustomListsRepository
import com.warrenbrowse.vpn.lib.repository.RelayListRepository

class CustomListRelayItemsUseCase(
    private val customListsRepository: CustomListsRepository,
    private val relayListRepository: RelayListRepository,
) {
    operator fun invoke(customListId: CustomListId): Flow<List<RelayItem.Location>> =
        combine(
            customListsRepository.customLists.mapNotNull { it?.getById(customListId) },
            relayListRepository.relayList,
        ) { customList, countries ->
            countries.getRelayItemsByCodes(customList.locations)
        }
}
