package com.warrenbrowse.vpn.feature.location.impl.search

import com.warrenbrowse.vpn.lib.model.RelayItem
import com.warrenbrowse.vpn.lib.model.RelayListType
import com.warrenbrowse.vpn.lib.ui.component.relaylist.RelayListItem
import com.warrenbrowse.vpn.lib.usecase.FilterChip

data class SearchLocationUiState(
    val searchTerm: String,
    val relayListType: RelayListType,
    val filterChips: List<FilterChip>,
    val relayListItems: List<RelayListItem>,
    val customLists: List<RelayItem.CustomList>,
)
