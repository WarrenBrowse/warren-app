package com.warrenbrowse.vpn.receiver

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import com.warrenbrowse.vpn.app.connect.holdLockedApps
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository
import org.koin.core.component.KoinComponent
import org.koin.core.component.inject

/**
 * Holds the apps locked to the VPN from boot, whether or not the tunnel is set
 * to connect then: a locked app started before Warren must not reach the bare
 * network (docs/app-routing.md section 8). A connect on boot replaces the
 * blackhole with the tunnel as any connect does.
 */
class LockedAppsBootCompletedReceiver : BroadcastReceiver(), KoinComponent {
    private val settings: WarrenLocalSettingsRepository by inject()

    override fun onReceive(context: Context?, intent: Intent?) {
        if (context == null || intent?.action != Intent.ACTION_BOOT_COMPLETED) return
        if (settings.lockedApps.value.isEmpty()) return
        holdLockedApps(context)
    }
}
