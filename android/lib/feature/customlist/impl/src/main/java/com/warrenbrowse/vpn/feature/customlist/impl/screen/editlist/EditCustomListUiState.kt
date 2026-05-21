package com.warrenbrowse.vpn.feature.customlist.impl.screen.editlist

import com.warrenbrowse.vpn.lib.model.CustomListId
import com.warrenbrowse.vpn.lib.model.CustomListName
import com.warrenbrowse.vpn.lib.model.GeoLocationId

sealed interface EditCustomListUiState {
    data object Loading : EditCustomListUiState

    data object NotFound : EditCustomListUiState

    data class Content(
        val id: CustomListId,
        val name: CustomListName,
        val locations: List<GeoLocationId>,
    ) : EditCustomListUiState
}
