package com.warrenbrowse.vpn.lib.repository

import arrow.core.Either
import arrow.core.right
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import com.warrenbrowse.vpn.lib.model.AddSplitTunnelingAppError
import com.warrenbrowse.vpn.lib.model.PackageName
import com.warrenbrowse.vpn.lib.model.RemoveSplitTunnelingAppError

// Split tunneling will become Warren-native (excludedPackages in the
// VpnService.Builder, persisted via WarrenLocalSettingsRepository) once the
// SplitTunneling screen is migrated. Until then this is a compile shim: state
// flows emit empty / disabled and mutators do nothing.
@Suppress("UNUSED_PARAMETER", "unused")
class SplitTunnelingRepository(
    @Suppress("UnusedPrivateMember") managementService: Any? = null,
) {
    val splitTunnelingEnabled: StateFlow<Boolean> = MutableStateFlow(false)

    val excludedApps: StateFlow<Set<PackageName>> = MutableStateFlow(emptySet())

    suspend fun enableSplitTunneling(
        enabled: Boolean,
    ): Either<RemoveSplitTunnelingAppError, Unit> = Unit.right()

    suspend fun excludeApp(app: PackageName): Either<AddSplitTunnelingAppError, Unit> =
        Unit.right()

    suspend fun includeApp(app: PackageName): Either<RemoveSplitTunnelingAppError, Unit> =
        Unit.right()
}
