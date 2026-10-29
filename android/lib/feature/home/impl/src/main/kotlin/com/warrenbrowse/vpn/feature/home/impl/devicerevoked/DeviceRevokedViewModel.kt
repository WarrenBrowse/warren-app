package com.warrenbrowse.vpn.feature.home.impl.devicerevoked

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.WhileSubscribed
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import com.warrenbrowse.vpn.lib.common.constant.VIEW_MODEL_STOP_TIMEOUT
import com.warrenbrowse.vpn.lib.model.DisconnectReason
import com.warrenbrowse.vpn.lib.repository.ConnectionProxy
import com.warrenbrowse.vpn.lib.repository.DeviceRepository

/**
 * The revoked screen (warren-core doc 115): the wallet already left the device when it shows, so
 * going to login only lifts whatever still holds the traffic and clears the revocation, which the
 * next login must not find.
 */
class DeviceRevokedViewModel(
    private val deviceRepository: DeviceRepository,
    private val connectionProxy: ConnectionProxy,
) : ViewModel() {

    val uiState =
        connectionProxy.tunnelState
            .map {
                if (it.isSecured()) {
                    DeviceRevokedUiState.SECURED
                } else {
                    DeviceRevokedUiState.UNSECURED
                }
            }
            .stateIn(
                scope = viewModelScope,
                started = SharingStarted.WhileSubscribed(VIEW_MODEL_STOP_TIMEOUT),
                initialValue = DeviceRevokedUiState.UNKNOWN,
            )

    private val _uiSideEffect = Channel<DeviceRevokedSideEffect>()
    val uiSideEffect = _uiSideEffect.receiveAsFlow()

    fun onGoToLoginClicked() {
        viewModelScope.launch {
            connectionProxy.disconnect(DisconnectReason.USER_INITIATED_GO_TO_LOGIN)
            deviceRepository.acknowledgeRevocation()
        }

        viewModelScope.launch { _uiSideEffect.send(DeviceRevokedSideEffect.NavigateToLogin) }
    }
}

sealed interface DeviceRevokedSideEffect {
    data object NavigateToLogin : DeviceRevokedSideEffect
}
