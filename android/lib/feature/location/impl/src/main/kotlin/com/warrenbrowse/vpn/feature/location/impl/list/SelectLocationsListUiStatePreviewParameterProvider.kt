package com.warrenbrowse.vpn.feature.location.impl.list

import androidx.compose.ui.tooling.preview.PreviewParameterProvider
import com.warrenbrowse.vpn.lib.common.Lce
import com.warrenbrowse.vpn.lib.model.MultihopRelayListType
import com.warrenbrowse.vpn.lib.model.RelayListType
import com.warrenbrowse.vpn.lib.ui.component.relaylist.RelayListItemPreviewData

class SelectLocationsListUiStatePreviewParameterProvider :
    PreviewParameterProvider<Lce<Unit, SelectLocationListUiState, Unit>> {
    override val values =
        sequenceOf(
            Lce.Content(
                SelectLocationListUiState(
                    relayListItems =
                        RelayListItemPreviewData.generateRelayListItems(
                            includeCustomLists = true,
                            isSearching = false,
                        ),
                    relayListType = RelayListType.Multihop(MultihopRelayListType.EXIT),
                )
            ),
            Lce.Loading(Unit),
            Lce.Error(Unit),
        )
}
