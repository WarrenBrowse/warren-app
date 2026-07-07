package com.warrenbrowse.vpn.lib.repository

import arrow.core.Either
import arrow.core.right
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import com.warrenbrowse.vpn.lib.model.AddSplitTunnelingAppError
import com.warrenbrowse.vpn.lib.model.PackageName
import com.warrenbrowse.vpn.lib.model.RemoveSplitTunnelingAppError

/**
 * Warren-native split tunnelling: the excluded set is persisted by
 * [WarrenLocalSettingsRepository] and applied by the tunnel service through
 * `VpnService.Builder.addDisallowedApplication` (see the service's TUN plan).
 * Excluding an app routes it OUTSIDE the tunnel.
 */
class SplitTunnelingRepository(
    private val settings: WarrenLocalSettingsRepository,
) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)

    val splitTunnelingEnabled: StateFlow<Boolean> = settings.splitTunnelingEnabled

    val excludedApps: StateFlow<Set<PackageName>> =
        settings.excludedApps
            .map { set -> set.map(::PackageName).toSet() }
            .stateIn(
                scope,
                SharingStarted.Eagerly,
                settings.excludedApps.value.map(::PackageName).toSet(),
            )

    suspend fun enableSplitTunneling(
        enabled: Boolean,
    ): Either<RemoveSplitTunnelingAppError, Unit> {
        settings.setSplitTunnelingEnabled(enabled)
        return Unit.right()
    }

    suspend fun excludeApp(app: PackageName): Either<AddSplitTunnelingAppError, Unit> {
        settings.addExcludedApp(app.value)
        return Unit.right()
    }

    suspend fun includeApp(app: PackageName): Either<RemoveSplitTunnelingAppError, Unit> {
        settings.removeExcludedApp(app.value)
        return Unit.right()
    }
}
