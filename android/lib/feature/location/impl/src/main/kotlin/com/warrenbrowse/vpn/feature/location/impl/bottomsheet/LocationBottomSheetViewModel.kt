package com.warrenbrowse.vpn.feature.location.impl.bottomsheet

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import co.touchlab.kermit.Logger
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.WhileSubscribed
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.flow.take
import kotlinx.coroutines.launch
import com.warrenbrowse.vpn.feature.location.api.LocationBottomSheetNavResult
import com.warrenbrowse.vpn.feature.location.api.LocationBottomSheetState
import com.warrenbrowse.vpn.feature.location.api.UndoChangeMultihopAction
import com.warrenbrowse.vpn.feature.location.impl.addLocationToCustomList
import com.warrenbrowse.vpn.feature.location.impl.removeLocationFromCustomList
import com.warrenbrowse.vpn.lib.common.Lc
import com.warrenbrowse.vpn.lib.common.constant.VIEW_MODEL_STOP_TIMEOUT
import com.warrenbrowse.vpn.lib.common.util.relaylist.withDescendants
import com.warrenbrowse.vpn.lib.model.CustomListId
import com.warrenbrowse.vpn.lib.model.CustomListName
import com.warrenbrowse.vpn.lib.model.GeoLocationId
import com.warrenbrowse.vpn.lib.model.RelayItem
import com.warrenbrowse.vpn.lib.repository.CustomListsRepository
import com.warrenbrowse.vpn.lib.repository.WireguardConstraintsRepository
import com.warrenbrowse.vpn.lib.usecase.HopSelectionUseCase
import com.warrenbrowse.vpn.lib.usecase.ModifyAndEnableMultihopUseCase
import com.warrenbrowse.vpn.lib.usecase.ModifyMultihopError
import com.warrenbrowse.vpn.lib.usecase.ModifyMultihopUseCase
import com.warrenbrowse.vpn.lib.usecase.MultihopChange
import com.warrenbrowse.vpn.lib.usecase.RelayItemCanBeSelectedUseCase
import com.warrenbrowse.vpn.lib.usecase.SelectAndEnableMultihopUseCase
import com.warrenbrowse.vpn.lib.usecase.SelectRelayItemError
import com.warrenbrowse.vpn.lib.usecase.SelectedLocationUseCase
import com.warrenbrowse.vpn.lib.usecase.customlists.CustomListActionUseCase
import com.warrenbrowse.vpn.lib.usecase.customlists.CustomListsRelayItemUseCase

@Suppress("TooManyFunctions", "LongParameterList")
class LocationBottomSheetViewModel(
    private val locationBottomSheetState: LocationBottomSheetState,
    private val customListActionUseCase: CustomListActionUseCase,
    private val customListsRepository: CustomListsRepository,
    private val hopSelectionUseCase: HopSelectionUseCase,
    private val modifyMultihopUseCase: ModifyMultihopUseCase,
    private val modifyAndEnableMultihopUseCase: ModifyAndEnableMultihopUseCase,
    private val selectAndEnableMultihopUseCase: SelectAndEnableMultihopUseCase,
    private val wireguardConstraintsRepository: WireguardConstraintsRepository,
    canBeSelectedUseCase: RelayItemCanBeSelectedUseCase,
    customListsRelayItemUseCase: CustomListsRelayItemUseCase,
    selectedLocationUseCase: SelectedLocationUseCase,
) : ViewModel() {
    val uiState: StateFlow<Lc<Unit, LocationBottomSheetUiState>> =
        combine(
                canBeSelectedUseCase(locationBottomSheetState.relayListType).take(1),
                customListsRelayItemUseCase(),
                selectedLocationUseCase().take(1),
            ) { canBeSelectedAs, customLists, selectedLocation ->
                when (locationBottomSheetState) {
                    is LocationBottomSheetState.ShowCustomListsEntryBottomSheet ->
                        Lc.Content(
                            LocationBottomSheetUiState.CustomListsEntry(
                                item = locationBottomSheetState.item,
                                setAsEntryState =
                                    canBeSelectedAs.entryIds?.validate(
                                        locationBottomSheetState.item
                                    ) ?: SetAsState.HIDDEN,
                                setAsExitState =
                                    canBeSelectedAs.exitIds?.validate(locationBottomSheetState.item)
                                        ?: SetAsState.HIDDEN,
                                // Custom list entries are never considered to be selected
                                canDisableMultihop = false,
                                customListId = locationBottomSheetState.customListId,
                                customListName =
                                    CustomListName.fromString(
                                        customLists
                                            .firstOrNull {
                                                it.id == locationBottomSheetState.customListId
                                            }
                                            ?.name ?: ""
                                    ),
                            )
                        )

                    is LocationBottomSheetState.ShowEditCustomListBottomSheet ->
                        Lc.Content(
                            LocationBottomSheetUiState.CustomList(
                                item = locationBottomSheetState.item,
                                setAsEntryState =
                                    canBeSelectedAs.entryIds?.validate(
                                        locationBottomSheetState.item
                                    ) ?: SetAsState.HIDDEN,
                                setAsExitState =
                                    canBeSelectedAs.exitIds?.validate(locationBottomSheetState.item)
                                        ?: SetAsState.HIDDEN,
                                canDisableMultihop =
                                    selectedLocation.entryLocation()?.getOrNull() ==
                                        locationBottomSheetState.item.id,
                            )
                        )

                    is LocationBottomSheetState.ShowLocationBottomSheet ->
                        Lc.Content(
                            LocationBottomSheetUiState.Location(
                                item = locationBottomSheetState.item,
                                customLists = customLists,
                                setAsEntryState =
                                    canBeSelectedAs.entryIds?.validate(
                                        locationBottomSheetState.item
                                    ) ?: SetAsState.HIDDEN,
                                setAsExitState =
                                    canBeSelectedAs.exitIds?.validate(locationBottomSheetState.item)
                                        ?: SetAsState.HIDDEN,
                                canDisableMultihop =
                                    selectedLocation.entryLocation()?.getOrNull() ==
                                        locationBottomSheetState.item.id,
                            )
                        )
                }
            }
            .stateIn(
                viewModelScope,
                started = SharingStarted.WhileSubscribed(VIEW_MODEL_STOP_TIMEOUT),
                Lc.Loading(Unit),
            )

    private val _uiSideEffect = Channel<LocationBottomSheetNavResult>()
    val uiSideEffect = _uiSideEffect.receiveAsFlow()

    fun setAsEntry(
        item: RelayItem,
        onError: (ModifyMultihopError, MultihopChange) -> Unit,
        onUpdateMultihop: (UndoChangeMultihopAction) -> Unit,
    ) {
        viewModelScope.launch(context = Dispatchers.IO) {
            val previousEntry =
                wireguardConstraintsRepository.wireguardConstraints.value
                    ?.entryLocation
                    ?.getOrNull()
            val change = MultihopChange.Entry(item)
            val isMultihopEnabled = isMultihopEnabled()
            if (isMultihopEnabled) {
                    modifyMultihopUseCase(change = change)
                } else {
                    modifyAndEnableMultihopUseCase(change = change, enableMultihop = true)
                }
                .fold(
                    { onError(it, change) },
                    {
                        if (!isMultihopEnabled) {
                            onUpdateMultihop(
                                if (previousEntry != null) {
                                    UndoChangeMultihopAction.DisableAndSetEntry(previousEntry)
                                } else {
                                    UndoChangeMultihopAction.Disable
                                }
                            )
                        }
                    },
                )
        }
    }

    fun setAsExit(
        item: RelayItem,
        onModifyMultihopError: (ModifyMultihopError, MultihopChange) -> Unit,
        onRelayItemError: (SelectRelayItemError) -> Unit,
        onUpdateMultihop: (UndoChangeMultihopAction) -> Unit,
    ) {
        viewModelScope.launch(context = Dispatchers.IO) {
            val previousExit = hopSelectionUseCase().first().exit()?.getOrNull()
            val isMultihopEnabled = isMultihopEnabled()
            if (isMultihopEnabled) {
                    modifyMultihopUseCase(MultihopChange.Exit(item = item))
                } else {
                    // If we are in singlehop mode we want to set a new multihop were the previous
                    // exit is set as an entry, and the new exit is set as exit. After that we turn
                    // on multihop
                    selectAndEnableMultihopUseCase(entry = previousExit, exit = item)
                }
                .fold(
                    { error ->
                        when (error) {
                            is ModifyMultihopError ->
                                onModifyMultihopError(error, MultihopChange.Exit(item))
                            is SelectRelayItemError -> onRelayItemError(error)
                            else -> error("Error not supported")
                        }
                    },
                    {
                        if (!isMultihopEnabled) {
                            onUpdateMultihop(
                                if (previousExit != null) {
                                    UndoChangeMultihopAction.DisableAndSetExit(previousExit.id)
                                } else {
                                    UndoChangeMultihopAction.Disable
                                }
                            )
                        }
                    },
                )
        }
    }

    fun disableMultihop(onUpdateMultihop: (UndoChangeMultihopAction) -> Unit) {
        viewModelScope.launch {
            wireguardConstraintsRepository
                .setMultihop(false)
                .fold(
                    { Logger.e("Set multihop error $it") },
                    { onUpdateMultihop(UndoChangeMultihopAction.Enable) },
                )
        }
    }

    fun addLocationToList(item: RelayItem.Location, customList: RelayItem.CustomList) {
        viewModelScope.launch {
            val result =
                addLocationToCustomList(
                    item = item,
                    customList = customList,
                    update = customListActionUseCase::invoke,
                )
            _uiSideEffect.send(LocationBottomSheetNavResult.CustomListActionToast(result))
        }
    }

    fun removeLocationFromList(item: RelayItem.Location, customListId: CustomListId) {
        viewModelScope.launch {
            val result =
                removeLocationFromCustomList(
                    item = item,
                    customListId = customListId,
                    getCustomListById = customListsRepository::getCustomListById,
                    update = customListActionUseCase::invoke,
                )
            _uiSideEffect.trySend(LocationBottomSheetNavResult.CustomListActionToast(result))
        }
    }

    fun onModifyMultihopError(
        modifyMultihopError: ModifyMultihopError,
        multihopChange: MultihopChange,
    ) {
        viewModelScope.launch {
            _uiSideEffect.send(modifyMultihopError.toSideEffect(multihopChange))
        }
    }

    fun onSelectRelayItemError(selectRelayItemError: SelectRelayItemError) {
        viewModelScope.launch { _uiSideEffect.send(selectRelayItemError.toSideEffect()) }
    }

    fun onMultihopChanged(undoChangeMultihopAction: UndoChangeMultihopAction) {
        viewModelScope.launch {
            _uiSideEffect.send(
                LocationBottomSheetNavResult.MultihopChanged(undoChangeMultihopAction)
            )
        }
    }

    private fun ModifyMultihopError.toSideEffect(
        multihopChange: MultihopChange
    ): LocationBottomSheetNavResult =
        when (this) {
            is ModifyMultihopError.EntrySameAsExit ->
                when (multihopChange) {
                    is MultihopChange.Entry ->
                        LocationBottomSheetNavResult.ExitAlreadySelected(relayItem = relayItem)
                    is MultihopChange.Exit ->
                        LocationBottomSheetNavResult.EntryAlreadySelected(relayItem = relayItem)
                }
            ModifyMultihopError.GenericError -> LocationBottomSheetNavResult.GenericError
            is ModifyMultihopError.RelayItemInactive ->
                LocationBottomSheetNavResult.RelayItemInactive(relayItem = relayItem)
        }

    private fun SelectRelayItemError.toSideEffect(): LocationBottomSheetNavResult =
        when (this) {
            SelectRelayItemError.GenericError -> LocationBottomSheetNavResult.GenericError
            is SelectRelayItemError.RelayInactive ->
                LocationBottomSheetNavResult.RelayItemInactive(relayItem = relayItem)
            SelectRelayItemError.EntryAndExitSame ->
                LocationBottomSheetNavResult.EntryAndExitAreSame
        }

    private fun isMultihopEnabled() =
        wireguardConstraintsRepository.wireguardConstraints.value?.isMultihopEnabled ?: false

    private fun Set<GeoLocationId>.validate(relayItem: RelayItem): SetAsState =
        if (
            when (relayItem) {
                is RelayItem.Location -> this.contains(relayItem.id)
                is RelayItem.CustomList ->
                    relayItem.locations.withDescendants().any { this.contains(it.id) }
            }
        ) {
            SetAsState.ENABLED
        } else {
            SetAsState.DISABLED
        }
}
