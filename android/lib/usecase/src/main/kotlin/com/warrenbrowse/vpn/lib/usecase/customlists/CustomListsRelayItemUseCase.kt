package com.warrenbrowse.vpn.lib.usecase.customlists

import kotlinx.coroutines.flow.combine
import com.warrenbrowse.vpn.lib.common.util.relaylist.toRelayItemCustomList
import com.warrenbrowse.vpn.lib.repository.CustomListsRepository
import com.warrenbrowse.vpn.lib.repository.RelayListRepository

class CustomListsRelayItemUseCase(
    private val customListsRepository: CustomListsRepository,
    private val relayListRepository: RelayListRepository,
) {

    operator fun invoke() =
        combine(customListsRepository.customLists, relayListRepository.relayList) {
            customLists,
            relayList ->
            customLists?.map { it.toRelayItemCustomList(relayList) } ?: emptyList()
        }
}
