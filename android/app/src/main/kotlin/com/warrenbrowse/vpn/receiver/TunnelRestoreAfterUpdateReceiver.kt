package com.warrenbrowse.vpn.receiver

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import co.touchlab.kermit.Logger
import com.warrenbrowse.vpn.lib.common.constant.KEY_CONNECT_ACTION
import com.warrenbrowse.vpn.lib.common.constant.VPN_SERVICE_CLASS
import com.warrenbrowse.vpn.lib.common.util.prepareVpnSafe
import com.warrenbrowse.vpn.lib.repository.UserPreferencesRepository
import com.warrenbrowse.vpn.receiver.util.goAsync
import com.warrenbrowse.vpn.receiver.util.shouldRestoreTunnelAfterUpdate
import org.koin.core.component.KoinComponent
import org.koin.core.component.inject

/** Starts the tunnel again after an update tore it down; see [shouldRestoreTunnelAfterUpdate]. */
class TunnelRestoreAfterUpdateReceiver : BroadcastReceiver(), KoinComponent {
    private val userPreferencesRepository by inject<UserPreferencesRepository>()

    override fun onReceive(context: Context, intent: Intent) {
        if (intent.action != Intent.ACTION_MY_PACKAGE_REPLACED) return
        goAsync {
            val prefs = userPreferencesRepository.preferences()
            val restore =
                shouldRestoreTunnelAfterUpdate(
                    tunnelRequested = prefs.tunnelRequested,
                    alwaysOnVpn = prefs.alwaysOnVpn,
                    hasVpnPermission = context.prepareVpnSafe().isRight(),
                )
            Logger.i("App updated; restoring the tunnel: $restore")
            if (restore) {
                // The same start the boot receiver makes: the service resolves
                // the wallet and the config itself, without the UI.
                context.startForegroundService(
                    Intent().apply {
                        setClassName(context.packageName, VPN_SERVICE_CLASS)
                        action = KEY_CONNECT_ACTION
                    }
                )
            }
        }
    }
}
