package com.warrenbrowse.vpn.lib.usecase.inappnotification

import com.warrenbrowse.vpn.lib.model.ActionAfterDisconnect
import com.warrenbrowse.vpn.lib.model.InAppNotification
import com.warrenbrowse.vpn.lib.model.TunnelState
import com.warrenbrowse.vpn.lib.repository.ConnectionProxy
import kotlin.time.Duration
import kotlin.time.Duration.Companion.seconds
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.transformLatest

/**
 * Upgrades the neutral "blocking internet" banner to a help hint once a connect
 * attempt has been running longer than a user is willing to wait. A stalled
 * attempt is almost always a real obstacle (captive portal, blocked UDP, a dead
 * exit), and the previous banner offered no door out of it.
 *
 * Pure UI timing: nothing here asks the engine anything, and the engine keeps
 * retrying underneath for as long as the banner is up.
 */
class ConnectingStuckNotificationUseCase(
    private val connectionProxy: ConnectionProxy,
    private val stuckAfter: Duration = STUCK_WINDOW,
) : InAppNotificationUseCase {

    @OptIn(ExperimentalCoroutinesApi::class)
    override operator fun invoke(): Flow<InAppNotification?> =
        connectionProxy.tunnelState
            // Collapsed to the phase first: a redial inside one attempt walks
            // through several states, and restarting the timer on each of them
            // would keep the banner permanently out of reach.
            .map { it.isConnectingPhase() }
            .distinctUntilChanged()
            .transformLatest { connecting ->
                emit(null)
                if (connecting) {
                    delay(stuckAfter)
                    emit(InAppNotification.ConnectingStuck)
                }
            }
            .distinctUntilChanged()

    private fun TunnelState.isConnectingPhase(): Boolean =
        when (this) {
            is TunnelState.Connecting -> true
            is TunnelState.Disconnecting ->
                actionAfterDisconnect == ActionAfterDisconnect.Reconnect
            else -> false
        }

    companion object {
        // Desktop useConnectingStuck window.
        val STUCK_WINDOW: Duration = 45.seconds
    }
}
