package com.warrenbrowse.vpn.feature.customlist.impl.screen.lists

import com.warrenbrowse.vpn.lib.model.CustomList

interface CustomListsUiState {
    object Loading : CustomListsUiState

    data class Content(val customLists: List<CustomList> = emptyList()) : CustomListsUiState
}
