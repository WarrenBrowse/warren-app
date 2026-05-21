package com.warrenbrowse.vpn.app.service

import com.warrenbrowse.vpn.lib.endpoint.ApiEndpointOverride

object WarrenDaemon {
    init {
        System.loadLibrary("mullvad_jni")
    }

    @Suppress("LongParameterList")
    external fun initialize(
        vpnService: WarrenVpnService,
        rpcSocketPath: String,
        filesDirectory: String,
        cacheDirectory: String,
        apiEndpointOverride: ApiEndpointOverride?,
        extraMetadata: Map<String, String>,
    )

    external fun shutdown()
}
