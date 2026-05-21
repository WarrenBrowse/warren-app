package com.warrenbrowse.vpn.lib.repository

import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import com.warrenbrowse.vpn.lib.grpc.ManagementService
import com.warrenbrowse.vpn.lib.model.PackageName

class SplitTunnelingRepository(
    private val managementService: ManagementService,
    dispatcher: CoroutineDispatcher = Dispatchers.IO,
) {
    val splitTunnelingEnabled =
        managementService.settings
            .map { it.splitTunnelSettings.enabled }
            .stateIn(CoroutineScope(dispatcher), SharingStarted.WhileSubscribed(), false)

    val excludedApps =
        managementService.settings
            .map { it.splitTunnelSettings.excludedApps }
            .stateIn(CoroutineScope(dispatcher), SharingStarted.WhileSubscribed(), emptySet())

    suspend fun enableSplitTunneling(enabled: Boolean) =
        managementService.setSplitTunnelingState(enabled)

    suspend fun excludeApp(app: PackageName) = managementService.addSplitTunnelingApp(app)

    suspend fun includeApp(app: PackageName) = managementService.removeSplitTunnelingApp(app)
}
