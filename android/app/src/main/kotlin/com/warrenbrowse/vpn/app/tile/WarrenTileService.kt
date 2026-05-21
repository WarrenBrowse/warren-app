package com.warrenbrowse.vpn.app.tile

import android.annotation.SuppressLint
import android.app.PendingIntent
import android.content.Intent
import android.graphics.drawable.Icon
import android.os.Build
import android.service.quicksettings.Tile
import android.service.quicksettings.TileService
import co.touchlab.kermit.Logger
import kotlinx.coroutines.Job
import kotlinx.coroutines.MainScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeoutOrNull
import com.warrenbrowse.vpn.lib.common.constant.MAIN_ACTIVITY_CLASS
import com.warrenbrowse.vpn.lib.common.util.getSupportedPendingIntentFlags
import com.warrenbrowse.vpn.lib.common.util.prepareVpnSafe
import com.warrenbrowse.vpn.lib.repository.WarrenQuinnDisconnectInvoker
import com.warrenbrowse.vpn.lib.repository.WarrenTunnelStateProvider
import com.warrenbrowse.vpn.lib.ui.resource.R
import org.koin.android.ext.android.get

class WarrenTileService : TileService() {
    private var job: Job? = null

    private lateinit var securedIcon: Icon
    private lateinit var unsecuredIcon: Icon

    // D.4 step 12: dead Mullvad ConnectionProxy + ManagementService
    // replaced by Warren-native surfaces. The tile reads state from the
    // process-singleton proxy (live mirror of WarrenQuinnAdapter.state)
    // and issues disconnect through the dedicated Warren use-case.
    // Connect from the tile requires the wallet UI (BiometricPrompt),
    // so a disconnected-tile click opens the main activity rather than
    // dispatching a connect intent directly.
    private val tunnelStateProvider = get<WarrenTunnelStateProvider>()
    private val warrenDisconnect = get<WarrenQuinnDisconnectInvoker>()

    override fun onCreate() {
        securedIcon = Icon.createWithResource(this, R.drawable.small_logo_white)
        unsecuredIcon = Icon.createWithResource(this, R.drawable.small_logo_black)
    }

    override fun onClick() {
        // Workaround for the reported bug: https://issuetracker.google.com/issues/236862865
        suspend fun isUnlockStatusPropagatedWithinTimeout(
            unlockTimeoutMillis: Long,
            unlockCheckDelayMillis: Long,
        ): Boolean {
            return withTimeoutOrNull(unlockTimeoutMillis) {
                while (isLocked) {
                    delay(unlockCheckDelayMillis)
                }
                return@withTimeoutOrNull true
            } ?: false
        }

        unlockAndRun {
            runBlocking {
                val isUnlockStatusPropagated =
                    isUnlockStatusPropagatedWithinTimeout(
                        unlockTimeoutMillis = 1000L,
                        unlockCheckDelayMillis = 100L,
                    )

                if (isUnlockStatusPropagated) {
                    toggleTunnel()
                } else {
                    Logger.e("Unable to toggle tunnel state")
                }
            }
        }
    }

    override fun onStartListening() {
        job = MainScope().launch { launchListenToTunnelState() }
    }

    override fun onStopListening() {
        job?.cancel()
    }

    @SuppressLint("StartActivityAndCollapseDeprecated")
    private fun toggleTunnel() {
        val isSetup = applicationContext.prepareVpnSafe().isRight()
        if (isSetup && qsTile.state == Tile.STATE_ACTIVE) {
            // Tile shows the tunnel as connected; user wants disconnect.
            Logger.i("TileService: dispatching Warren disconnect")
            warrenDisconnect.disconnect()
        } else {
            // Either VPN profile is not set up yet, or tunnel is
            // disconnected. The Warren connect path needs the wallet's
            // BiometricPrompt host (FragmentActivity), so we bounce the
            // user into the main activity rather than starting the
            // service directly.
            Logger.i("TileService: opening main activity for Warren connect")
            val intent =
                Intent().apply {
                    setClassName(applicationContext.packageName, MAIN_ACTIVITY_CLASS)
                    flags =
                        Intent.FLAG_ACTIVITY_CLEAR_TOP or
                            Intent.FLAG_ACTIVITY_SINGLE_TOP or
                            Intent.FLAG_ACTIVITY_NEW_TASK
                    action = Intent.ACTION_MAIN
                }
            startActivityAndCollapseCompat(intent)
        }
    }

    @SuppressLint("StartActivityAndCollapseDeprecated")
    private fun WarrenTileService.startActivityAndCollapseCompat(intent: Intent) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            val pendingIntent =
                PendingIntent.getActivity(
                    applicationContext,
                    0,
                    intent,
                    getSupportedPendingIntentFlags(),
                )
            startActivityAndCollapse(pendingIntent)
        } else {
            @Suppress("DEPRECATION") startActivityAndCollapse(intent)
        }
    }

    private suspend fun launchListenToTunnelState() {
        tunnelStateProvider.state
            .map(::mapToTileState)
            .collect { updateTileState(it) }
    }

    private fun mapToTileState(stateLabel: String): Int =
        when {
            // The proxy exposes a String projection; "Connected",
            // "Connecting...", "Reconnecting..." all map to the active
            // tile (the tunnel intent is captured). Anything else is
            // inactive.
            stateLabel.startsWith("Connected") -> Tile.STATE_ACTIVE
            stateLabel.startsWith("Connecting") -> Tile.STATE_ACTIVE
            stateLabel.startsWith("Reconnecting") -> Tile.STATE_ACTIVE
            else -> Tile.STATE_INACTIVE
        }

    private fun updateTileState(newState: Int) {
        qsTile?.apply {
            if (newState == Tile.STATE_ACTIVE) {
                state = Tile.STATE_ACTIVE
                icon = securedIcon
                label = resources.getString(R.string.app_name)
                setSubtitleIfSupported(resources.getText(R.string.connected))
            } else {
                state = Tile.STATE_INACTIVE
                icon = unsecuredIcon
                label = resources.getString(R.string.app_name)
                setSubtitleIfSupported(resources.getText(R.string.disconnected))
            }
            updateTile()
        }
    }

    private fun Tile.setSubtitleIfSupported(subtitleText: CharSequence) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            this.subtitle = subtitleText
        }
    }

}
