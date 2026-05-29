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

// D.4 step 58: RelayListRepository stripped of ManagementService +
// RelayLocationTranslationRepository deps. The Mullvad daemon's relay channel
// is dead on Warren — Warren reads the relay catalogue directly through
// `WarrenRelayProvider` (warren-api-client / WarrenJni.listRelays) and stores
// the user's selection in `WarrenLocalSettingsRepository.selectedExitId`.
//
// The shim here keeps consumers that still reference the Mullvad-shaped API
// (RelayItem.Location.Country list, Constraint<RelayItemId>) compiling while
// they are being migrated. All read flows emit empty / Any defaults ; all
// mutators are no-ops returning Right(Unit). SelectedLocationTitleUseCase
// (only production consumer that mattered) already returns null on empty
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
