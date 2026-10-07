package com.warrenbrowse.vpn.app.connect

import android.content.Context
import android.content.Intent
import co.touchlab.kermit.Logger
import com.warrenbrowse.vpn.app.service.WarrenVpnService
import com.warrenbrowse.vpn.lib.common.constant.KEY_HOLD_LOCKED_APPS_ACTION
import com.warrenbrowse.vpn.lib.common.util.prepareVpnSafe

/**
 * Starts the tunnel service to hold the apps locked to the VPN behind their own
 * blackhole (docs/app-routing.md section 8). Without the VPN permission no
 * interface can be established, so nothing is started.
 */
fun holdLockedApps(context: Context) {
    if (context.prepareVpnSafe().isLeft()) {
        Logger.w("Locked apps not held: VPN permission not granted")
        return
    }
    val intent =
        Intent(context, WarrenVpnService::class.java).apply {
            action = KEY_HOLD_LOCKED_APPS_ACTION
        }
    try {
        context.startForegroundService(intent)
    } catch (e: IllegalStateException) {
        Logger.e(throwable = e) { "Could not start the service to hold the locked apps" }
    }
}
