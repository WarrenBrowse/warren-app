package com.warrenbrowse.vpn.lib.usecase.customlists

import kotlin.collections.map
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.map
import com.warrenbrowse.vpn.lib.common.util.relaylist.toRelayItemCustomList
import com.warrenbrowse.vpn.lib.model.RelayListType
import com.warrenbrowse.vpn.lib.repository.CustomListsRepository
import com.warrenbrowse.vpn.lib.usecase.FilteredRelayListUseCase

class FilterCustomListsRelayItemUseCase(
    private val customListsRepository: CustomListsRepository,
    private val filteredRelayListUseCase: FilteredRelayListUseCase,
) {

    operator fun invoke(relayListType: RelayListType) =
        combine(customListsRepository.customLists, filteredRelayListUseCase(relayListType)) {
            customLists,
            filteredRelayList ->
            customLists?.map { it.toRelayItemCustomList(filteredRelayList) } ?: emptyList()
        }
}
