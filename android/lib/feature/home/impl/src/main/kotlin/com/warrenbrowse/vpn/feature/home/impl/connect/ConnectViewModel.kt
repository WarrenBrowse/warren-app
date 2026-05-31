package com.warrenbrowse.vpn.feature.home.impl.connect

import android.net.Uri
import androidx.core.net.toUri
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.FlowPreview
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.WhileSubscribed
import kotlinx.coroutines.flow.filterIsInstance
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.merge
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import com.warrenbrowse.vpn.feature.applisting.api.ResolveAppListingUseCase
import com.warrenbrowse.vpn.feature.home.impl.connect.notificationbanner.InAppNotificationController
import com.warrenbrowse.vpn.lib.common.constant.VIEW_MODEL_STOP_TIMEOUT
import com.warrenbrowse.vpn.lib.common.util.combine
import com.warrenbrowse.vpn.lib.common.util.withPrev
import com.warrenbrowse.vpn.lib.model.ActionAfterDisconnect
import com.warrenbrowse.vpn.lib.model.DeviceState
import com.warrenbrowse.vpn.lib.model.PrepareError
import com.warrenbrowse.vpn.lib.model.TunnelState
import com.warrenbrowse.vpn.lib.repository.ChangelogRepository
import com.warrenbrowse.vpn.lib.repository.ConnectionProxy
import com.warrenbrowse.vpn.lib.repository.DeviceRepository
import com.warrenbrowse.vpn.lib.repository.UserPreferencesRepository
import com.warrenbrowse.vpn.lib.repository.WarrenQuinnDisconnectInvoker
import com.warrenbrowse.vpn.lib.repository.WarrenQuinnReconnectInvoker
import com.warrenbrowse.vpn.lib.usecase.LastKnownLocationUseCase
import com.warrenbrowse.vpn.lib.usecase.SelectedLocationTitleUseCase
import com.warrenbrowse.vpn.lib.usecase.SystemVpnSettingsAvailableUseCase

@Suppress("LongParameterList")
class ConnectViewModel(
    private val deviceRepository: DeviceRepository,
    private val changelogRepository: ChangelogRepository,
    inAppNotificationController: InAppNotificationController,
    private val userPreferencesRepository: UserPreferencesRepository,
    selectedLocationTitleUseCase: SelectedLocationTitleUseCase,
    private val connectionProxy: ConnectionProxy,
    lastKnownLocationUseCase: LastKnownLocationUseCase,
    private val systemVpnSettingsUseCase: SystemVpnSettingsAvailableUseCase,
    private val warrenDisconnect: WarrenQuinnDisconnectInvoker,
    private val warrenReconnect: WarrenQuinnReconnectInvoker,
    private val isPlayBuild: Boolean,
    private val resolveAppListing: ResolveAppListingUseCase,
) : ViewModel() {
    private val _uiSideEffect = Channel<UiSideEffect>()

    val uiSideEffect =
        merge(_uiSideEffect.receiveAsFlow(), revokedDeviceEffect())

    @OptIn(FlowPreview::class)
    val uiState: StateFlow<ConnectUiState> =
        combine(
                selectedLocationTitleUseCase(),
                inAppNotificationController.notifications,
                connectionProxy.tunnelState.withPrev(),
                lastKnownLocationUseCase.lastKnownDisconnectedLocation,
            ) {
                selectedRelayItemTitle,
                notifications,
                (tunnelState, prevTunnelState),
                lastKnownDisconnectedLocation ->
                ConnectUiState(
                    location =
                        when (tunnelState) {
                            is TunnelState.Disconnected ->
                                tunnelState.location ?: lastKnownDisconnectedLocation

                            is TunnelState.Connecting -> tunnelState.location
                            is TunnelState.Connected -> tunnelState.location
                            is TunnelState.Disconnecting ->
                                when (tunnelState.actionAfterDisconnect) {
                                    ActionAfterDisconnect.Nothing -> lastKnownDisconnectedLocation
                                    ActionAfterDisconnect.Block -> lastKnownDisconnectedLocation
                                    // Keep the previous connected location when reconnecting, after
                                    // this state we will reach Connecting with the new relay
                                    // location
                                    ActionAfterDisconnect.Reconnect -> prevTunnelState?.location()
                                }

                            is TunnelState.Error -> lastKnownDisconnectedLocation
                        },
                    selectedRelayItemTitle =
                        if (tunnelState is TunnelState.Disconnected) {
                            selectedRelayItemTitle
                        } else {
                            null
                        },
                    tunnelState = tunnelState,
                    inAppNotification = notifications.firstOrNull(),
                    isPlayBuild = isPlayBuild,
                )
            }
            .stateIn(
                viewModelScope,
                SharingStarted.WhileSubscribed(VIEW_MODEL_STOP_TIMEOUT),
                ConnectUiState.INITIAL,
            )

    init {
        // D.4 step 36: Mullvad Play Store purchase verification dropped (Warren
        // identity is BIP39 wallet, no VPN subscription billing).
        viewModelScope.launch { deviceRepository.updateDevice() }
    }

    fun onDisconnectClick() {
        viewModelScope.launch {
            // D.4 step 60: route disconnect through WarrenQuinnDisconnectInvoker
            // (real Warren tunnel teardown). The legacy ConnectionProxy.disconnect
            // is now a no-op stub.
            warrenDisconnect.disconnect()
        }
    }

    fun onReconnectClick() {
        viewModelScope.launch {
            // D.4 step 60: WarrenQuinnReconnectInvoker (real Warren reconnect).
            warrenReconnect.reconnect()
        }
    }

    fun onConnectClick() {
        // D.4 step 60: connect is dispatched from `ConnectScreen` directly
        // through `WarrenQuinnConnectInvoker.connect(activity)` since the
        // invoker needs a FragmentActivity for biometric unlock — the
        // ViewModel can only emit a side effect requesting the screen to
        // perform the dispatch. The legacy connectionProxy.connect() call
        // here was a no-op stub anyway.
        viewModelScope.launch {
            _uiSideEffect.send(UiSideEffect.RequestWarrenConnect)
        }
    }

    fun createVpnProfileResult(hasVpnPermission: Boolean) {
        viewModelScope.launch {
            if (hasVpnPermission) {
                // D.4 step 60: VPN permission granted → request UI to dispatch
                // WarrenQuinnConnectInvoker.connect(activity).
                _uiSideEffect.send(UiSideEffect.RequestWarrenConnect)
            } else {
                // Either the user denied the permission or another always-on-vpn is active (if
                // Android 11+ and run from Android Studio)
                // If we don't have vpn system settings available we assume that there is no other
                // always-on-vpn active.
                _uiSideEffect.send(
                    UiSideEffect.ConnectError.PermissionDenied(systemVpnSettingsUseCase())
                )
            }
        }
    }

    fun onCancelClick() {
        viewModelScope.launch {
            // D.4 step 60: cancel-in-progress-connect = disconnect (Warren has
            // no separate Cancel command; tear down the in-flight tunnel).
            warrenDisconnect.disconnect()
        }
    }

    // D.4 step 43: onManageAccountClick + OpenAccountManagementPageInBrowser
    // side effect removed entirely (mullvad.net web-account flow dead).
    // ConnectScreen routes Manage Account taps directly to
    // WarrenWalletSettings via NavKey now.

    fun openAppListing() = viewModelScope.launch {
        val target = resolveAppListing()
        val sideEffect =
            UiSideEffect.OpenUri(
                uri = target.listingUri.toUri(),
                errorMessage = target.errorMessage,
            )
        _uiSideEffect.send(sideEffect)
    }

    // D.4 step 41: dismissNewDeviceNotification removed (NewDeviceRepository dead).

    fun dismissAndroid16UpgradeWarning() = viewModelScope.launch {
        userPreferencesRepository.setShowAndroid16ConnectWarning(false)
    }

    fun dismissNewChangelogNotification() = viewModelScope.launch {
        changelogRepository.setDismissNewChangelogNotification()
    }

    private fun revokedDeviceEffect() =
        deviceRepository.deviceState.filterIsInstance<DeviceState.Revoked>().map {
            UiSideEffect.RevokedDevice
        }

    sealed interface UiSideEffect {
        data class OpenUri(val uri: Uri, val errorMessage: String) : UiSideEffect

        data object RevokedDevice : UiSideEffect

        // D.4 step 60: connect dispatch needs a FragmentActivity for biometric
        // unlock, so the VM emits this side effect and ConnectScreen invokes
        // WarrenQuinnConnectInvoker.connect(activity) on the current Activity.
        data object RequestWarrenConnect : UiSideEffect

        data class NotPrepared(val prepareError: PrepareError) : UiSideEffect

        sealed interface ConnectError : UiSideEffect {
            data object Generic : ConnectError

            data class PermissionDenied(val systemVpnSettingsAvailable: Boolean) : ConnectError
        }
    }
}
