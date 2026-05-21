package com.warrenbrowse.vpn.lib.repository

import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import com.warrenbrowse.vpn.lib.grpc.ManagementService
import com.warrenbrowse.vpn.lib.model.BuildVersion
import com.warrenbrowse.vpn.lib.model.VersionInfo

class AppVersionInfoRepository(
    private val buildVersion: BuildVersion,
    managementService: ManagementService,
    dispatcher: CoroutineDispatcher = Dispatchers.IO,
) {
    val versionInfo: StateFlow<VersionInfo> =
        managementService.versionInfo
            .map { appVersionInfo ->
                VersionInfo(
                    currentVersion = buildVersion.name,
                    isSupported = appVersionInfo.supported,
                )
            }
            .stateIn(
                CoroutineScope(dispatcher),
                SharingStarted.WhileSubscribed(),
                // By default we assume we are supported
                VersionInfo(currentVersion = buildVersion.name, isSupported = true),
            )
}
