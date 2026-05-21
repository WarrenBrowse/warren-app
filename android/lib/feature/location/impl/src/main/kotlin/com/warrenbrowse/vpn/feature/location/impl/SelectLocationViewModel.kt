package com.warrenbrowse.vpn.feature.location.impl

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.WhileSubscribed
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.filterNotNull
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import com.warrenbrowse.vpn.feature.location.api.UndoChangeMultihopAction
import com.warrenbrowse.vpn.lib.common.Lc
import com.warrenbrowse.vpn.lib.common.constant.VIEW_MODEL_STOP_TIMEOUT
import com.warrenbrowse.vpn.lib.common.util.combine
import com.warrenbrowse.vpn.lib.common.util.isEntryAndBlocked
import com.warrenbrowse.vpn.lib.common.util.isMultihopEnabled
import com.warrenbrowse.vpn.lib.model.Constraint
import com.warrenbrowse.vpn.lib.model.MultihopRelayListType
import com.warrenbrowse.vpn.lib.model.Recents
import com.warrenbrowse.vpn.lib.model.RelayItem
import com.warrenbrowse.vpn.lib.model.RelayListType
import com.warrenbrowse.vpn.lib.model.Settings
import com.warrenbrowse.vpn.lib.model.TunnelState
import com.warrenbrowse.vpn.lib.model.communication.CustomListAction
import com.warrenbrowse.vpn.lib.repository.ConnectionProxy
import com.warrenbrowse.vpn.lib.repository.RelayListFilterRepository
import com.warrenbrowse.vpn.lib.repository.RelayListRepository
import com.warrenbrowse.vpn.lib.repository.SettingsRepository
import com.warrenbrowse.vpn.lib.repository.WireguardConstraintsRepository
import com.warrenbrowse.vpn.lib.usecase.FilterChipUseCase
import com.warrenbrowse.vpn.lib.usecase.HopSelectionUseCase
import com.warrenbrowse.vpn.lib.usecase.ModifyMultihopError
import com.warrenbrowse.vpn.lib.usecase.ModifyMultihopUseCase
import com.warrenbrowse.vpn.lib.usecase.MultihopChange
import com.warrenbrowse.vpn.lib.usecase.SelectRelayItemError
import com.warrenbrowse.vpn.lib.usecase.SelectSinglehopUseCase
import com.warrenbrowse.vpn.lib.usecase.customlists.CustomListActionUseCase

@OptIn(ExperimentalCoroutinesApi::class)
@Suppress("TooManyFunctions", "LongParameterList")
class SelectLocationViewModel(
    private val relayListFilterRepository: RelayListFilterRepository,
    private val customListActionUseCase: CustomListActionUseCase,
    private val relayListRepository: RelayListRepository,
    private val wireguardConstraintsRepository: WireguardConstraintsRepository,
    private val filterChipUseCase: FilterChipUseCase,
    private val settingsRepository: SettingsRepository,
    private val selectSingleUseCase: SelectSinglehopUseCase,
    private val modifyMultihopUseCase: ModifyMultihopUseCase,
    private val relayListScrollConnection: RelayListScrollConnection,
    hopSelectionUseCase: HopSelectionUseCase,
    connectionProxy: ConnectionProxy,
) : ViewModel() {
    private val _multihopRelayListTypeSelection: MutableStateFlow<MultihopRelayListType> =
        MutableStateFlow(MultihopRelayListType.EXIT)

    val uiState =
        combine(
                filterChips(),
                _multihopRelayListTypeSelection.filterNotNull(),
                relayListRepository.relayList,
                settingsRepository.settingsUpdates.filterNotNull(),
                connectionProxy.tunnelState
                    .map { it as? TunnelState.Error }
                    .map { it?.errorState?.cause },
                hopSelectionUseCase(),
            ) { filterChips, relayListSelection, relayList, settings, errorStateCause, selectedHop
                ->
                Lc.Content(
                    SelectLocationUiState(
                        filterChips = filterChips,
                        multihopListSelection = relayListSelection,
                        isSearchButtonEnabled =
                            searchButtonEnabled(
                                relayList = relayList,
                                relayListSelection = relayListSelection,
                                settings = settings,
                            ),
                        isFilterButtonEnabled = relayList.isNotEmpty(),
                        isRecentsEnabled = settings.recents is Recents.Enabled,
                        hopSelection = selectedHop,
                        tunnelErrorStateCause = errorStateCause,
                    )
                )
            }
            .stateIn(
                viewModelScope,
                SharingStarted.WhileSubscribed(VIEW_MODEL_STOP_TIMEOUT),
                Lc.Loading(Unit),
            )

    private val _uiSideEffect = Channel<SelectLocationSideEffect>()
    val uiSideEffect = _uiSideEffect.receiveAsFlow()

    private fun filterChips() =
        combine(settingsRepository.settingsUpdates, _multihopRelayListTypeSelection) {
                settings,
                multihopRelayListType ->
                if (settings?.isMultihopEnabled() == true)
                    RelayListType.Multihop(multihopRelayListType)
                else RelayListType.Single
            }
            .flatMapLatest { filterChipUseCase(it) }

    private fun searchButtonEnabled(
        relayList: List<RelayItem.Location.Country>,
        relayListSelection: MultihopRelayListType,
        settings: Settings,
    ): Boolean {
        val hasRelayListItems = relayList.isNotEmpty()
        val isEntryAndBlocked =
            isEntryAndBlocked(multihopRelayListType = relayListSelection, settings = settings)
        return hasRelayListItems && !isEntryAndBlocked
    }

    fun selectRelayList(multihopRelayListType: MultihopRelayListType) {
        viewModelScope.launch { _multihopRelayListTypeSelection.emit(multihopRelayListType) }
    }

    fun selectSingle(item: RelayItem) {
        viewModelScope.launch {
            selectSingleUseCase(item)
                .fold(
                    { _uiSideEffect.send(it.toSideEffect()) },
                    { _uiSideEffect.send(SelectLocationSideEffect.CloseScreen) },
                )
        }
    }

    fun modifyMultihop(relayItem: RelayItem, multihopRelayListType: MultihopRelayListType) {
        val change =
            when (multihopRelayListType) {
                MultihopRelayListType.ENTRY -> MultihopChange.Entry(relayItem)
                MultihopRelayListType.EXIT -> MultihopChange.Exit(relayItem)
            }

        viewModelScope.launch { modifyMultihop(change = change) }
    }

    private suspend fun modifyMultihop(change: MultihopChange) {
        modifyMultihopUseCase(change)
            .fold(
                { _uiSideEffect.send(it.toSideEffect(change)) },
                {
                    when (change) {
                        is MultihopChange.Entry ->
                            _multihopRelayListTypeSelection.emit(MultihopRelayListType.EXIT)

                        is MultihopChange.Exit ->
                            _uiSideEffect.send(SelectLocationSideEffect.CloseScreen)
                    }
                },
            )
    }

    private fun ModifyMultihopError.toSideEffect(
        multihopChange: MultihopChange
    ): SelectLocationSideEffect =
        when (this) {
            is ModifyMultihopError.EntrySameAsExit ->
                when (multihopChange) {
                    is MultihopChange.Entry ->
                        SelectLocationSideEffect.ExitAlreadySelected(relayItem = relayItem)

                    is MultihopChange.Exit ->
                        SelectLocationSideEffect.EntryAlreadySelected(relayItem = relayItem)
                }

            ModifyMultihopError.GenericError -> SelectLocationSideEffect.GenericError
            is ModifyMultihopError.RelayItemInactive ->
                SelectLocationSideEffect.RelayItemInactive(relayItem = relayItem)
        }

    private fun SelectRelayItemError.toSideEffect(): SelectLocationSideEffect =
        when (this) {
            SelectRelayItemError.GenericError -> SelectLocationSideEffect.GenericError
            is SelectRelayItemError.RelayInactive ->
                SelectLocationSideEffect.RelayItemInactive(relayItem = relayItem)

            SelectRelayItemError.EntryAndExitSame -> SelectLocationSideEffect.EntryAndExitAreSame
        }

    fun performAction(action: CustomListAction) {
        viewModelScope.launch { customListActionUseCase(action) }
    }

    fun removeOwnerFilter() {
        viewModelScope.launch { relayListFilterRepository.updateSelectedOwnership(Constraint.Any) }
    }

    fun removeProviderFilter() {
        viewModelScope.launch { relayListFilterRepository.updateSelectedProviders(Constraint.Any) }
    }

    fun toggleRecentsEnabled() {
        viewModelScope.launch {
            val enabled = settingsRepository.settingsUpdates.value?.recents is Recents.Enabled
            settingsRepository.setRecentsEnabled(!enabled)
        }
    }

    fun refreshRelayList() {
        viewModelScope.launch {
            relayListRepository.refreshRelayList()
            _uiSideEffect.send(SelectLocationSideEffect.RelayListUpdating)
        }
    }

    fun toggleMultihop(enable: Boolean) {
        viewModelScope.launch {
            wireguardConstraintsRepository
                .setMultihop(enable)
                .fold(
                    { _uiSideEffect.send(SelectLocationSideEffect.GenericError) },
                    {
                        if (enable) {
                            _multihopRelayListTypeSelection.emit(MultihopRelayListType.EXIT)
                        }
                    },
                )
        }
    }

    fun undoMultihopAction(undoChangeMultihopAction: UndoChangeMultihopAction) {
        viewModelScope.launch {
            when (undoChangeMultihopAction) {
                UndoChangeMultihopAction.Enable ->
                    wireguardConstraintsRepository.setMultihop(true).onLeft {
                        _uiSideEffect.send(SelectLocationSideEffect.GenericError)
                    }

                UndoChangeMultihopAction.Disable ->
                    wireguardConstraintsRepository.setMultihop(false).onLeft {
                        _uiSideEffect.send(SelectLocationSideEffect.GenericError)
                    }

                is UndoChangeMultihopAction.DisableAndSetEntry ->
                    wireguardConstraintsRepository
                        .setMultihopAndEntryLocation(false, undoChangeMultihopAction.relayItemId)
                        .onLeft { _uiSideEffect.send(SelectLocationSideEffect.GenericError) }

                is UndoChangeMultihopAction.DisableAndSetExit ->
                    relayListRepository
                        .updateExitRelayLocationMultihop(
                            false,
                            undoChangeMultihopAction.relayItemId,
                        )
                        .onLeft { _uiSideEffect.send(SelectLocationSideEffect.GenericError) }
            }
        }
    }

    fun scrollToItem(event: ScrollEvent) {
        relayListScrollConnection.scrollEvents.trySend(event)
    }
}

sealed interface SelectLocationSideEffect {
    data object CloseScreen : SelectLocationSideEffect

    data object GenericError : SelectLocationSideEffect

    data class RelayItemInactive(val relayItem: RelayItem) : SelectLocationSideEffect

    data class EntryAlreadySelected(val relayItem: RelayItem) : SelectLocationSideEffect

    data class ExitAlreadySelected(val relayItem: RelayItem) : SelectLocationSideEffect

    data object EntryAndExitAreSame : SelectLocationSideEffect

    data object RelayListUpdating : SelectLocationSideEffect
}
