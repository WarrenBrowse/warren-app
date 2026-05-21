package com.warrenbrowse.vpn.feature.location.impl.list

import com.warrenbrowse.vpn.lib.model.RelayListType
import com.warrenbrowse.vpn.lib.ui.component.relaylist.RelayListItem

data class SelectLocationListUiState(
    val relayListType: RelayListType,
    val relayListItems: List<RelayListItem>,
)
