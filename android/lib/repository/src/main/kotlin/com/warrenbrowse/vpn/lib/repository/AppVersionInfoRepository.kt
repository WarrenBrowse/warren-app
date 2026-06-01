package com.warrenbrowse.vpn.lib.repository

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import com.warrenbrowse.vpn.lib.model.BuildVersion
import com.warrenbrowse.vpn.lib.model.VersionInfo

// Emits a constant `isSupported = true` so the "unsupported version" in-app
// banner never fires. A Warren-native equivalent (poll warren-api
// `/v1/version`) is planned.
class AppVersionInfoRepository(private val buildVersion: BuildVersion) {
    val versionInfo: StateFlow<VersionInfo> =
        MutableStateFlow(VersionInfo(currentVersion = buildVersion.name, isSupported = true))
}
