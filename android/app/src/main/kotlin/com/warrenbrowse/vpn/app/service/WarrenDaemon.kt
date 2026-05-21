package com.warrenbrowse.vpn.app.service

import com.warrenbrowse.vpn.lib.endpoint.ApiEndpointOverride

// Legacy Mullvad daemon shim. Warren does not run a full state-machine daemon
// inside the JNI library; the new entry point is `com.warrenbrowse.vpn.jni.WarrenJni`
// (cf. `warren-jni/src/lib.rs`). This object will be deleted once D.4 wires
// `WarrenVpnService` directly against `WarrenJni`. Kept here as a no-op for the
// transitional D.3 commit so existing call sites still compile while the
// rewrite proceeds.
object WarrenDaemon {
    init {
        System.loadLibrary("warren_jni")
    }

    @Suppress("LongParameterList", "UnusedParameter")
    fun initialize(
        vpnService: WarrenVpnService,
        rpcSocketPath: String,
        filesDirectory: String,
        cacheDirectory: String,
        apiEndpointOverride: ApiEndpointOverride?,
        extraMetadata: Map<String, String>,
    ) {
        // TODO (D.4): drop this shim and call WarrenJni.connectTunnel from
        // WarrenVpnService instead.
    }

    fun shutdown() {
        // TODO (D.4): WarrenJni.disconnectTunnel()
    }
}
