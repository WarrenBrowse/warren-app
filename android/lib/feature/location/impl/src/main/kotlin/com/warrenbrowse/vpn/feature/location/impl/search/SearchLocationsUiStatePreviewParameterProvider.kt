package com.warrenbrowse.vpn.feature.location.impl.search

import androidx.compose.ui.tooling.preview.PreviewParameterProvider
import com.warrenbrowse.vpn.lib.common.Lce
import com.warrenbrowse.vpn.lib.model.MultihopRelayListType
import com.warrenbrowse.vpn.lib.model.RelayListType
import com.warrenbrowse.vpn.lib.ui.component.relaylist.RelayListItemPreviewData
import com.warrenbrowse.vpn.lib.usecase.FilterChip

class SearchLocationsUiStatePreviewParameterProvider :
    PreviewParameterProvider<Lce<Unit, SearchLocationUiState, Unit>> {
    override val values =
        sequenceOf(
            Lce.Loading(Unit),
            Lce.Content(
                SearchLocationUiState(
                    searchTerm = "",
                    filterChips = listOf(FilterChip.Entry),
                    relayListItems =
                        RelayListItemPreviewData.generateRelayListItems(
                            includeCustomLists = true,
                            isSearching = true,
                        ),
                    customLists = emptyList(),
                    relayListType = RelayListType.Multihop(MultihopRelayListType.ENTRY),
                )
            ),
            Lce.Error(Unit),
            Lce.Content(
                SearchLocationUiState(
                    searchTerm = "Mullvad",
                    filterChips = listOf(FilterChip.Entry),
                    relayListItems =
                        RelayListItemPreviewData.generateEmptyList("Mullvad", isSearching = true),
                    customLists = emptyList(),
                    relayListType = RelayListType.Multihop(MultihopRelayListType.ENTRY),
                )
            ),
            Lce.Content(
                SearchLocationUiState(
                    searchTerm = "Germany",
                    filterChips = listOf(FilterChip.Entry),
                    relayListItems =
                        RelayListItemPreviewData.generateRelayListItems(
                            includeCustomLists = true,
                            isSearching = true,
                        ),
                    customLists = emptyList(),
                    relayListType = RelayListType.Multihop(MultihopRelayListType.ENTRY),
                )
            ),
        )
}
