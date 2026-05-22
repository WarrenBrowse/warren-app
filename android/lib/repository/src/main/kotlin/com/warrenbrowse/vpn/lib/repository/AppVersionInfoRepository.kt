package com.warrenbrowse.vpn.lib.repository

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import com.warrenbrowse.vpn.lib.model.BuildVersion
import com.warrenbrowse.vpn.lib.model.VersionInfo

// D.4 step 58: AppVersionInfoRepository stripped of ManagementService dep.
// The Mullvad daemon's `versionInfo` channel reported whether the running
// client was still supported by the Mullvad API — dead on Warren since the
// upstream daemon is gone. The Warren-native equivalent (poll warren-api
// `/v1/version`) will land in D.6 ; until then we emit a constant
// `isSupported = true` so the "unsupported version" in-app banner never
// fires.
class AppVersionInfoRepository(private val buildVersion: BuildVersion) {
    val versionInfo: StateFlow<VersionInfo> =
        MutableStateFlow(VersionInfo(currentVersion = buildVersion.name, isSupported = true))
}
