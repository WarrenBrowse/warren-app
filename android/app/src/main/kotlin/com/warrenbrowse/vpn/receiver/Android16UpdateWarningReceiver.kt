package com.warrenbrowse.vpn.receiver

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.os.Build
import android.provider.Settings
import com.warrenbrowse.vpn.lib.repository.UserPreferencesRepository
import com.warrenbrowse.vpn.receiver.util.AlwaysOnVpnGuidance
import com.warrenbrowse.vpn.receiver.util.alwaysOnVpnGuidance
import com.warrenbrowse.vpn.receiver.util.goAsync
import kotlin.getValue
import org.koin.core.component.KoinComponent
import org.koin.core.component.inject

class Android16UpdateWarningReceiver : BroadcastReceiver(), KoinComponent {
    private val userPreferencesRepository by inject<UserPreferencesRepository>()

    override fun onReceive(context: Context?, intent: Intent?) {
        if (intent?.action == Intent.ACTION_MY_PACKAGE_REPLACED) {
            // Check that we run Android 16 (Baklava)
            goAsync {
                userPreferencesRepository.setShowAndroid16ConnectWarning(
                    Build.VERSION.SDK_INT == Build.VERSION_CODES.BAKLAVA
                )
                context?.let { adviseOnAlwaysOnVpn(it) }
            }
        }
    }

    /**
     * The app was just replaced, which is the one window it cannot protect: the
     * OS tears the tunnel down and no app may hold the network closed while it
     * is being swapped. The desktop installers arm a lockdown and stage a
     * detached guard; the mobile equivalent is a system setting the user has to
     * turn on, so all this can do is notice it is off and say so once.
     */
    private suspend fun adviseOnAlwaysOnVpn(context: Context) {
        val guidance =
            alwaysOnVpnGuidance(
                alwaysOnVpnPackage = readSecure(context, ALWAYS_ON_VPN_APP),
                lockdownEnabled = readSecure(context, ALWAYS_ON_VPN_LOCKDOWN)?.let { it == "1" },
                ourPackage = context.packageName,
            )
        if (guidance == AlwaysOnVpnGuidance.NOT_CONFIGURED) {
            userPreferencesRepository.setShowAlwaysOnVpnAdvice(true)
        }
    }

    /**
     * These keys are not public API, so a device may hide them or throw. `null`
     * means "could not read", which the verdict treats as a reason to stay
     * silent rather than to warn.
     */
    private fun readSecure(context: Context, key: String): String? =
        runCatching { Settings.Secure.getString(context.contentResolver, key) }.getOrNull()

    private companion object {
        const val ALWAYS_ON_VPN_APP = "always_on_vpn_app"
        const val ALWAYS_ON_VPN_LOCKDOWN = "always_on_vpn_lockdown"
    }
}
