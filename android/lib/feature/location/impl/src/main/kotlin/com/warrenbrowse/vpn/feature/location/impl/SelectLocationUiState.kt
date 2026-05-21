package com.warrenbrowse.vpn.feature.location.impl

import com.warrenbrowse.vpn.lib.model.ErrorStateCause
import com.warrenbrowse.vpn.lib.model.HopSelection
import com.warrenbrowse.vpn.lib.model.MultihopRelayListType
import com.warrenbrowse.vpn.lib.model.RelayListType
import com.warrenbrowse.vpn.lib.usecase.FilterChip

data class SelectLocationUiState(
    val filterChips: List<FilterChip>,
    val multihopListSelection: MultihopRelayListType,
    val isSearchButtonEnabled: Boolean,
    val isFilterButtonEnabled: Boolean,
    val isRecentsEnabled: Boolean,
    val hopSelection: HopSelection,
    val tunnelErrorStateCause: ErrorStateCause?,
) {
    val multihopEnabled: Boolean = hopSelection is HopSelection.Multi
    val relayListType =
        if (multihopEnabled) RelayListType.Multihop(multihopListSelection) else RelayListType.Single
}
