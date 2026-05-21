package com.warrenbrowse.vpn.lib.usecase.inappnotification

import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.map
import com.warrenbrowse.vpn.lib.common.util.inAnyOf
import com.warrenbrowse.vpn.lib.model.ActionAfterDisconnect
import com.warrenbrowse.vpn.lib.model.Constraint
import com.warrenbrowse.vpn.lib.model.ErrorState
import com.warrenbrowse.vpn.lib.model.ErrorStateCause
import com.warrenbrowse.vpn.lib.model.InAppNotification
import com.warrenbrowse.vpn.lib.model.ParameterGenerationError
import com.warrenbrowse.vpn.lib.model.Port
import com.warrenbrowse.vpn.lib.model.PortRange
import com.warrenbrowse.vpn.lib.model.Settings
import com.warrenbrowse.vpn.lib.model.TunnelState
import com.warrenbrowse.vpn.lib.repository.ConnectionProxy
import com.warrenbrowse.vpn.lib.repository.RelayListRepository
import com.warrenbrowse.vpn.lib.repository.SettingsRepository

class TunnelStateNotificationUseCase(
    private val connectionProxy: ConnectionProxy,
    private val relayListRepository: RelayListRepository,
    private val settingsRepository: SettingsRepository,
) : InAppNotificationUseCase {
    @OptIn(ExperimentalCoroutinesApi::class)
    override operator fun invoke(): Flow<InAppNotification?> =
        connectionProxy.tunnelState
            .distinctUntilChanged()
            .map(::tunnelStateNotification)
            .flatMapLatest { inAppNotification ->
                combine(relayListRepository.portRanges, settingsRepository.settingsUpdates) {
                    portRanges,
                    settings ->
                    inAppNotification?.maybeUpdateWithPortError(
                        wireguardPort = settings.wireguardPort(),
                        availablePorts = portRanges,
                    )
                }
            }
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

    private fun InAppNotification.maybeUpdateWithPortError(
        wireguardPort: Constraint<Port>,
        availablePorts: List<PortRange>,
    ): InAppNotification =
        if (this is InAppNotification.TunnelStateError && error.isPossiblePortError()) {
            wireguardPort.invalidPortOrNull(availablePorts)?.let {
                copy(
                    error =
                        ErrorState(
                            cause = ErrorStateCause.NoRelaysMatchSelectedPort(port = it),
                            isBlocking = error.isBlocking,
                        )
                )
            } ?: this
        } else this

    private fun ErrorState.isPossiblePortError(): Boolean =
        (cause as? ErrorStateCause.TunnelParameterError)?.error?.let {
            it == ParameterGenerationError.NoMatchingRelayEntry ||
                it == ParameterGenerationError.NoMatchingRelayExit ||
                it == ParameterGenerationError.NoMatchingRelay
        } ?: false

    private fun Constraint<Port>.invalidPortOrNull(availablePortRanges: List<PortRange>): Port? =
        getOrNull()?.takeIf { !it.inAnyOf(availablePortRanges) }

    private fun Settings?.wireguardPort() =
        this?.obfuscationSettings?.wireguardPort ?: Constraint.Any
}
