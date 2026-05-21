package com.warrenbrowse.vpn.feature.location.impl.list

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.WhileSubscribed
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.filter
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.flow.stateIn
import com.warrenbrowse.vpn.feature.location.impl.RelayListScrollConnection
import com.warrenbrowse.vpn.feature.location.impl.onToggleExpandSet
import com.warrenbrowse.vpn.feature.location.impl.search.emptyLocationsRelayListItems
import com.warrenbrowse.vpn.feature.location.impl.search.relayListItems
import com.warrenbrowse.vpn.feature.location.impl.search.selectedByOtherEntryExitList
import com.warrenbrowse.vpn.feature.location.impl.search.selectedByThisEntryExitList
import com.warrenbrowse.vpn.lib.common.Lce
import com.warrenbrowse.vpn.lib.common.constant.VIEW_MODEL_STOP_TIMEOUT
import com.warrenbrowse.vpn.lib.common.util.ignoreEntrySelection
import com.warrenbrowse.vpn.lib.common.util.isEntryAndBlocked
import com.warrenbrowse.vpn.lib.model.CustomListId
import com.warrenbrowse.vpn.lib.model.GeoLocationId
import com.warrenbrowse.vpn.lib.model.MultihopRelayListType
import com.warrenbrowse.vpn.lib.model.RelayItem
import com.warrenbrowse.vpn.lib.model.RelayItemId
import com.warrenbrowse.vpn.lib.model.RelayListType
import com.warrenbrowse.vpn.lib.repository.RelayListRepository
import com.warrenbrowse.vpn.lib.repository.SettingsRepository
import com.warrenbrowse.vpn.lib.repository.WireguardConstraintsRepository
import com.warrenbrowse.vpn.lib.ui.component.relaylist.RelayListItem
import com.warrenbrowse.vpn.lib.usecase.FilteredRelayListUseCase
import com.warrenbrowse.vpn.lib.usecase.RecentsUseCase
import com.warrenbrowse.vpn.lib.usecase.SelectedLocationUseCase
import com.warrenbrowse.vpn.lib.usecase.customlists.FilterCustomListsRelayItemUseCase

@Suppress("LongParameterList")
class SelectLocationListViewModel(
    private val relayListType: RelayListType,
    private val filteredRelayListUseCase: FilteredRelayListUseCase,
    private val filteredCustomListRelayItemsUseCase: FilterCustomListsRelayItemUseCase,
    private val selectedLocationUseCase: SelectedLocationUseCase,
    private val wireguardConstraintsRepository: WireguardConstraintsRepository,
    private val relayListRepository: RelayListRepository,
    private val recentsUseCase: RecentsUseCase,
    private val settingsRepository: SettingsRepository,
    relayListScrollConnection: RelayListScrollConnection,
) : ViewModel() {
    private val _expandedItems: MutableStateFlow<Set<String>> =
        MutableStateFlow(initialExpand(initialSelection()))

    val uiState: StateFlow<Lce<Unit, SelectLocationListUiState, Unit>> =
        combine(relayListItems(), settingsRepository.settingsUpdates) { relayListItems, settings ->
                if (relayListType.isEntryAndBlocked(settings)) {
                    Lce.Error(Unit)
                } else {
                    Lce.Content(
                        SelectLocationListUiState(
                            relayListType = relayListType,
                            relayListItems = relayListItems,
                        )
                    )
                }
            }
            .stateIn(
                viewModelScope,
                SharingStarted.WhileSubscribed(VIEW_MODEL_STOP_TIMEOUT),
                Lce.Loading(Unit),
            )
    val uiSideEffect =
        relayListScrollConnection.scrollEvents
            .receiveAsFlow()
            .filter { it.first == relayListType }
            .map { ScrollSideEffect(it.second) }

    fun onToggleExpand(item: RelayItemId, parent: CustomListId? = null, expand: Boolean) {
        _expandedItems.onToggleExpandSet(item, parent, expand)
    }

    private fun relayListItems(): Flow<List<RelayListItem>> =
        combine(
            filteredRelayListUseCase(relayListType = relayListType),
            filteredCustomListRelayItemsUseCase(relayListType = relayListType),
            recentsUseCase(relayListType = relayListType),
            selectedLocationUseCase(),
            _expandedItems,
        ) { relayCountries, customLists, recents, selectedItem, expandedItems ->
            // If we have no locations we have an empty relay list and we should show an error
            if (relayCountries.isEmpty()) {
                emptyLocationsRelayListItems(
                    relayListType = relayListType,
                    customLists = customLists,
                    selectedByThisEntryExitList =
                        selectedItem.selectedByThisEntryExitList(relayListType),
                    selectedByOtherEntryExitList =
                        selectedItem.selectedByOtherEntryExitList(relayListType, customLists),
                    expandedItems = expandedItems,
                )
            } else {
                val settings = settingsRepository.settingsUpdates.value
                relayListItems(
                    relayCountries = relayCountries,
                    relayListType = relayListType,
                    customLists = customLists,
                    recents = recents,
                    selectedItem = selectedItem,
                    selectedByThisEntryExitList =
                        selectedItem.selectedByThisEntryExitList(relayListType),
                    selectedByOtherEntryExitList =
                        if (ignoreEntrySelection(settings, relayListType)) {
                            null
                        } else {
                            selectedItem.selectedByOtherEntryExitList(relayListType, customLists)
                        },
                    expandedItems = expandedItems,
                )
            }
        }

    private fun initialExpand(item: RelayItemId?): Set<String> = buildSet {
        when (item) {
            is GeoLocationId.City -> {
                add(item.country.code)
            }
            is GeoLocationId.Hostname -> {
                add(item.country.code)
                add(item.city.code)
            }
            is CustomListId,
            is GeoLocationId.Country,
            null -> {
                /* No expands */
            }
        }
    }

    private fun initialSelection() =
        when (relayListType) {
            RelayListType.Single -> relayListRepository.selectedLocation.value
            is RelayListType.Multihop ->
                when (relayListType.multihopRelayListType) {
                    MultihopRelayListType.ENTRY ->
                        wireguardConstraintsRepository.wireguardConstraints.value?.entryLocation
                    MultihopRelayListType.EXIT -> relayListRepository.selectedLocation.value
                }
        }?.getOrNull()
}

data class ScrollSideEffect(val relayItem: RelayItem)
