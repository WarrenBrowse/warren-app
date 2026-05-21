package com.warrenbrowse.vpn.app.service

import java.io.File
import com.warrenbrowse.vpn.lib.endpoint.ApiEndpointOverride

data class DaemonConfig(
    val rpcSocket: File,
    val filesDir: File,
    val cacheDir: File,
    val apiEndpointOverride: ApiEndpointOverride?,
)
