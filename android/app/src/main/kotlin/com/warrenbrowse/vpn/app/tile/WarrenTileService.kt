package com.warrenbrowse.vpn.app.tile

import android.annotation.SuppressLint
import android.app.PendingIntent
import android.content.Intent
import android.graphics.drawable.Icon
import android.os.Build
import android.service.quicksettings.Tile
import android.service.quicksettings.TileService
import co.touchlab.kermit.Logger
import com.warrenbrowse.vpn.lib.common.constant.KEY_CONNECT_ACTION
import com.warrenbrowse.vpn.lib.common.constant.MAIN_ACTIVITY_CLASS
import com.warrenbrowse.vpn.lib.common.constant.VPN_SERVICE_CLASS
import com.warrenbrowse.vpn.lib.common.util.getSupportedPendingIntentFlags
import com.warrenbrowse.vpn.lib.common.util.prepareVpnSafe
import com.warrenbrowse.vpn.lib.model.wallet.WalletState
import com.warrenbrowse.vpn.lib.repository.WalletRepository
import com.warrenbrowse.vpn.lib.repository.WarrenQuinnDisconnectInvoker
import com.warrenbrowse.vpn.lib.repository.WarrenTunnelStateProvider
import com.warrenbrowse.vpn.lib.ui.resource.R
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.MainScope
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull
import org.koin.android.ext.android.get

class WarrenTileService : TileService() {
    private val scope: CoroutineScope = MainScope()
    private var job: Job? = null

    private lateinit var securedIcon: Icon
    private lateinit var unsecuredIcon: Icon

    // The tile reads the typed tunnel state from the process-singleton proxy (live mirror of
    // WarrenQuinnAdapter.state) and stops the tunnel through the Warren use case. A connect starts
    // the service the way the boot receiver does: the service reads the wallet itself, without a
    // prompt.
    private val tunnelStateProvider = get<WarrenTunnelStateProvider>()
    private val warrenDisconnect = get<WarrenQuinnDisconnectInvoker>()
    private val wallet = get<WalletRepository>()

    override fun onCreate() {
        securedIcon = Icon.createWithResource(this, R.drawable.small_logo_white)
        unsecuredIcon = Icon.createWithResource(this, R.drawable.small_logo_black)
    }

    override fun onDestroy() {
        scope.cancel()
        super.onDestroy()
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

        // The wait for the unlock to propagate suspends instead of holding the main thread, which
        // the system UI shares with this callback.
        unlockAndRun {
            scope.launch {
                val isUnlockStatusPropagated =
                    isUnlockStatusPropagatedWithinTimeout(
                        unlockTimeoutMillis = UNLOCK_TIMEOUT_MS,
                        unlockCheckDelayMillis = UNLOCK_CHECK_DELAY_MS,
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
        job?.cancel()
        job = scope.launch { launchListenToTunnelState() }
    }

    override fun onStopListening() {
        job?.cancel()
    }

    @SuppressLint("StartActivityAndCollapseDeprecated")
    private fun toggleTunnel() {
        val isSetup = applicationContext.prepareVpnSafe().isRight()
        when {
            qsTile?.state == Tile.STATE_ACTIVE -> {
                Logger.i("TileService: dispatching Warren disconnect")
                warrenDisconnect.disconnect()
            }
            isSetup && wallet.state.value !is WalletState.Absent -> {
                Logger.i("TileService: starting the tunnel")
                startForegroundService(
                    Intent().apply {
                        setClassName(applicationContext.packageName, VPN_SERVICE_CLASS)
                        action = KEY_CONNECT_ACTION
                    }
                )
            }
            else -> {
                // The VPN permission or the wallet is missing: both are granted in the app.
                Logger.i("TileService: opening main activity to set up the connection")
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
        tunnelStateProvider.connectedInfo.map(::tileStatusFor).collect { updateTileState(it) }
    }

    private fun updateTileState(status: TileStatus) {
        qsTile?.apply {
            state = if (status.active) Tile.STATE_ACTIVE else Tile.STATE_INACTIVE
            icon = if (status.active) securedIcon else unsecuredIcon
            label = resources.getString(R.string.app_name)
            setSubtitleIfSupported(resources.getText(status.subtitle()))
            updateTile()
        }
    }

    private fun TileStatus.subtitle(): Int =
        when (this) {
            TileStatus.Connected -> R.string.connected
            TileStatus.Connecting -> R.string.connecting
            TileStatus.Blocking -> R.string.blocking_internet
            TileStatus.Disconnected -> R.string.disconnected
        }

    private fun Tile.setSubtitleIfSupported(subtitleText: CharSequence) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            this.subtitle = subtitleText
        }
    }

    private companion object {
        const val UNLOCK_TIMEOUT_MS = 1000L
        const val UNLOCK_CHECK_DELAY_MS = 100L
    }
}
