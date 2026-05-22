package com.warrenbrowse.vpn.lib.usecase.inappnotification

import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map
import com.warrenbrowse.vpn.lib.model.ActionAfterDisconnect
import com.warrenbrowse.vpn.lib.model.InAppNotification
import com.warrenbrowse.vpn.lib.model.TunnelState
import com.warrenbrowse.vpn.lib.repository.ConnectionProxy

// D.4 step 54: TunnelStateNotificationUseCase simplified — drop SettingsRepository
// + RelayListRepository.portRanges + maybeUpdateWithPortError. The Mullvad
// "WireGuard port out of available range" hint only made sense with the Mullvad
// daemon's wireguard_port obfuscation setting. Warren uses Quinn over UDP/443
// and has no equivalent port selector ; just surface the tunnel state error
// straight through.
class TunnelStateNotificationUseCase(private val connectionProxy: ConnectionProxy) :
    InAppNotificationUseCase {
    override operator fun invoke(): Flow<InAppNotification?> =
        connectionProxy.tunnelState
            .distinctUntilChanged()
            .map(::tunnelStateNotification)
            .distinctUntilChanged()

    private fun tunnelStateNotification(tunnelUiState: TunnelState): InAppNotification? =
        when (tunnelUiState) {
            is TunnelState.Connecting -> InAppNotification.TunnelStateBlocked
            is TunnelState.Disconnecting -> {
                if (
                    tunnelUiState.actionAfterDisconnect == ActionAfterDisconnect.Block ||
                        tunnelUiState.actionAfterDisconnect == ActionAfterDisconnect.Reconnect
                ) {
                    InAppNotification.TunnelStateBlocked
                } else null
            }
            is TunnelState.Error -> InAppNotification.TunnelStateError(tunnelUiState.errorState)
            is TunnelState.Connected,
            is TunnelState.Disconnected -> null
        }
}
