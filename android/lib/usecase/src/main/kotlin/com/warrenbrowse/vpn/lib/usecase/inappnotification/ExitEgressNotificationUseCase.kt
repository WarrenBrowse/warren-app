package com.warrenbrowse.vpn.lib.usecase.inappnotification

import com.warrenbrowse.vpn.lib.model.InAppNotification
import com.warrenbrowse.vpn.lib.model.TunnelState
import com.warrenbrowse.vpn.lib.repository.ConnectionProxy
import com.warrenbrowse.vpn.lib.repository.WarrenPathHealthProvider
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged

/**
 * Names the cause when the tunnel is up and carrying nothing: the host network
 * is fine, the exit stopped forwarding.
 *
 * The connect card ORs this verdict with the offline one to degrade its status,
 * which is right for the status line but loses the distinction the user needs
 * to know whether to change network or wait for an automatic switch. The banner
 * is where the two causes stay apart.
 *
 * The tunnel-connected gate is load-bearing: the wedge verdict outlives the
 * session it was measured on, so without it a torn down tunnel would leave the
 * banner on screen.
 */
class ExitEgressNotificationUseCase(
    private val pathHealthProvider: WarrenPathHealthProvider,
    private val connectionProxy: ConnectionProxy,
) : InAppNotificationUseCase {
    override operator fun invoke(): Flow<InAppNotification?> =
        combine(pathHealthProvider.pathWedged, connectionProxy.tunnelState) { wedged, tunnelState ->
                if (wedged && tunnelState is TunnelState.Connected) {
                    InAppNotification.ExitEgressDead
                } else {
                    null
                }
            }
            .distinctUntilChanged()
}
