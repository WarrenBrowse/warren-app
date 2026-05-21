package com.warrenbrowse.vpn.feature.customlist.impl.screen.editlocations

import com.warrenbrowse.vpn.lib.common.Lce
import com.warrenbrowse.vpn.lib.ui.component.relaylist.CheckableRelayListItem

data class CustomListLocationsUiState(
    val newList: Boolean,
    val content: Lce<Unit, CustomListLocationsData, Unit>,
)

data class CustomListLocationsData(
    val saveEnabled: Boolean,
    val hasUnsavedChanges: Boolean,
    val searchTerm: String,
    val locations: List<CheckableRelayListItem>,
)
