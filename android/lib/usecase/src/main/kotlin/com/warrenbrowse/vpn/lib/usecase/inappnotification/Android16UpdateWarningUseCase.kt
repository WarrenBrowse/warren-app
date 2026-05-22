package com.warrenbrowse.vpn.lib.usecase.inappnotification

import kotlin.time.Duration.Companion.seconds
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.transformLatest
import com.warrenbrowse.vpn.lib.model.ActionAfterDisconnect
import com.warrenbrowse.vpn.lib.model.InAppNotification
import com.warrenbrowse.vpn.lib.model.TunnelState
import com.warrenbrowse.vpn.lib.repository.ConnectionProxy
import com.warrenbrowse.vpn.lib.repository.UserPreferencesRepository

// D.4 step 58: Android16UpdateWarningUseCase rewired to ConnectionProxy (Warren
// stub) instead of the dead Mullvad ManagementService. Since ConnectionProxy
// emits a constant `Disconnected` tunnel state until the Warren-native tunnel
// state plumbing replaces it (D.4 step 67+), the warning never fires in
// practice — kept here as a code-path placeholder.
class Android16UpdateWarningUseCase(
    private val userPreferencesRepository: UserPreferencesRepository,
    private val connectionProxy: ConnectionProxy,
) : InAppNotificationUseCase {
    @OptIn(ExperimentalCoroutinesApi::class)
    override operator fun invoke(): Flow<InAppNotification?> =
        combine(
                userPreferencesRepository.showAndroid16ConnectWarning().distinctUntilChanged(),
                connectionProxy.tunnelState.map { it.toTunState() }.distinctUntilChanged(),
            ) { showWarning, tunState ->
                showWarning to tunState
            }
            .transformLatest { (showWarning, tunState) ->
                when {
                    showWarning && tunState == SimpleTunState.Connecting -> {
                        emit(null)
                        delay(SHOW_WARNING_DELAY)
                        emit(InAppNotification.Android16UpgradeWarning)
                    }
                    showWarning && tunState == SimpleTunState.Connected -> {
                        // User is connected, we know this warning is not relevant so we remove it
                        // and don't show the warning again.
                        userPreferencesRepository.setShowAndroid16ConnectWarning(false)
                        emit(null)
                    }
                    else -> emit(null)
                }
            }

    private fun TunnelState.toTunState(): SimpleTunState =
        when (this) {
            is TunnelState.Connecting -> SimpleTunState.Connecting
            is TunnelState.Disconnecting if
                actionAfterDisconnect == ActionAfterDisconnect.Reconnect
             -> SimpleTunState.Connecting
            is TunnelState.Connected -> SimpleTunState.Connected
            else -> SimpleTunState.Other
        }

    private enum class SimpleTunState {
        Connecting,
        Connected,
        Other,
    }

    companion object {
        private val SHOW_WARNING_DELAY = 2.seconds
    }
}
