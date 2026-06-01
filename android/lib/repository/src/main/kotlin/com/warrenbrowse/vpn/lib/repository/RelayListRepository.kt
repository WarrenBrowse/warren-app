package com.warrenbrowse.vpn.lib.repository

import arrow.core.Either
import arrow.core.right
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.flowOf
import com.warrenbrowse.vpn.lib.model.Constraint
import com.warrenbrowse.vpn.lib.model.GeoLocationId
import com.warrenbrowse.vpn.lib.model.PortRange
import com.warrenbrowse.vpn.lib.model.RelayItem
import com.warrenbrowse.vpn.lib.model.RelayItemId
import com.warrenbrowse.vpn.lib.model.SetRelayLocationError
import com.warrenbrowse.vpn.lib.model.UpdateRelayLocationsError

// Warren reads the relay catalogue directly through `WarrenRelayProvider`
// (warren-api-client / WarrenJni.listRelays) and stores the user's selection
// in `WarrenLocalSettingsRepository.selectedExitId`.
//
// This shim keeps consumers that reference the Mullvad-shaped API
// (RelayItem.Location.Country list, Constraint<RelayItemId>) compiling. All
// read flows emit empty / Any defaults; all mutators return Right(Unit)
// without doing anything. SelectedLocationTitleUseCase returns null on empty
// inputs.
@Suppress("UNUSED_PARAMETER", "unused")
class RelayListRepository(
    @Suppress("UnusedPrivateMember") managementService: Any? = null,
    @Suppress("UnusedPrivateMember") translationRepository: Any? = null,
) {
    val relayList: StateFlow<List<RelayItem.Location.Country>> = MutableStateFlow(emptyList())

    val selectedLocation: StateFlow<Constraint<RelayItemId>> = MutableStateFlow(Constraint.Any)

    val portRanges: Flow<List<PortRange>> = flowOf(emptyList())

    val shadowsocksPortRanges: Flow<List<PortRange>> = flowOf(emptyList())

    suspend fun updateSelectedRelayLocation(value: RelayItemId): Either<SetRelayLocationError, Unit> =
        Unit.right()

    suspend fun updateSelectedRelayLocationMultihop(
        isMultihopEnabled: Boolean,
        entry: RelayItemId,
        exit: RelayItemId,
    ): Either<SetRelayLocationError, Unit> = Unit.right()

    suspend fun updateExitRelayLocationMultihop(
        isMultihopEnabled: Boolean,
        exit: RelayItemId,
    ): Either<SetRelayLocationError, Unit> = Unit.right()

    suspend fun refreshRelayList(): Either<UpdateRelayLocationsError, Unit> = Unit.right()

    fun find(geoLocationId: GeoLocationId): RelayItem.Location? = null
}
