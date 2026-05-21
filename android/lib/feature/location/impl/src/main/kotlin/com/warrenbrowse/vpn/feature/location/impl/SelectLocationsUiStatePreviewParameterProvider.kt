package com.warrenbrowse.vpn.feature.location.impl

import androidx.compose.ui.tooling.preview.PreviewParameterProvider
import com.warrenbrowse.vpn.lib.common.Lc
import com.warrenbrowse.vpn.lib.common.toLc
import com.warrenbrowse.vpn.lib.model.HopSelection
import com.warrenbrowse.vpn.lib.model.MultihopRelayListType
import com.warrenbrowse.vpn.lib.usecase.FilterChip
import com.warrenbrowse.vpn.lib.usecase.ModelOwnership

class SelectLocationsUiStatePreviewParameterProvider :
    PreviewParameterProvider<Lc<Unit, SelectLocationUiState>> {
    override val values =
        sequenceOf(
            Lc.Loading(Unit),
            SelectLocationUiState(
                    filterChips = emptyList(),
                    multihopListSelection = MultihopRelayListType.EXIT,
                    isSearchButtonEnabled = true,
                    isFilterButtonEnabled = true,
                    isRecentsEnabled = true,
                    hopSelection = HopSelection.Single(null),
                    tunnelErrorStateCause = null,
                )
                .toLc(),
            SelectLocationUiState(
                    filterChips =
                        listOf(
                            FilterChip.Ownership(ownership = ModelOwnership.Rented),
                            FilterChip.Provider(PROVIDER_COUNT),
                        ),
                    multihopListSelection = MultihopRelayListType.EXIT,
                    isSearchButtonEnabled = true,
                    isFilterButtonEnabled = true,
                    isRecentsEnabled = true,
                    hopSelection = HopSelection.Single(null),
                    tunnelErrorStateCause = null,
                )
                .toLc(),
            SelectLocationUiState(
                    filterChips = emptyList(),
                    multihopListSelection = MultihopRelayListType.ENTRY,
                    isSearchButtonEnabled = true,
                    isFilterButtonEnabled = true,
                    isRecentsEnabled = true,
                    hopSelection = HopSelection.Multi(null, null),
                    tunnelErrorStateCause = null,
                )
                .toLc(),
            SelectLocationUiState(
                    filterChips =
                        listOf(
                            FilterChip.Ownership(ownership = ModelOwnership.MullvadOwned),
                            FilterChip.Provider(PROVIDER_COUNT),
                        ),
                    multihopListSelection = MultihopRelayListType.ENTRY,
                    isSearchButtonEnabled = true,
                    isFilterButtonEnabled = true,
                    isRecentsEnabled = true,
                    hopSelection = HopSelection.Multi(null, null),
                    tunnelErrorStateCause = null,
                )
                .toLc(),
        )
}

private const val PROVIDER_COUNT = 3
