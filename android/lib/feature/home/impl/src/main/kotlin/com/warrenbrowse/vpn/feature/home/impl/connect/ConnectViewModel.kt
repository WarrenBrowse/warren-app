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
import kotlinx.coroutines.flow.filter
import kotlinx.coroutines.flow.filterIsInstance
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.merge
import kotlinx.coroutines.flow.onStart
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import com.warrenbrowse.vpn.feature.applisting.api.ResolveAppListingUseCase
import com.warrenbrowse.vpn.feature.home.impl.connect.notificationbanner.InAppNotificationController
import com.warrenbrowse.vpn.lib.common.constant.VIEW_MODEL_STOP_TIMEOUT
import com.warrenbrowse.vpn.lib.common.util.combine
import com.warrenbrowse.vpn.lib.common.util.daysLeft
import com.warrenbrowse.vpn.lib.common.util.withPrev
import com.warrenbrowse.vpn.lib.model.ActionAfterDisconnect
import com.warrenbrowse.vpn.lib.model.ConnectError
import com.warrenbrowse.vpn.lib.model.DeviceState
import com.warrenbrowse.vpn.lib.model.DisconnectReason
import com.warrenbrowse.vpn.lib.model.PrepareError
import com.warrenbrowse.vpn.lib.model.TunnelState
import com.warrenbrowse.vpn.lib.model.WebsiteAuthToken
import com.warrenbrowse.vpn.lib.payment.model.VerificationResult
import com.warrenbrowse.vpn.lib.repository.AccountRepository
import com.warrenbrowse.vpn.lib.repository.ChangelogRepository
import com.warrenbrowse.vpn.lib.repository.ConnectionProxy
import com.warrenbrowse.vpn.lib.repository.DeviceRepository
import com.warrenbrowse.vpn.lib.repository.NewDeviceRepository
import com.warrenbrowse.vpn.lib.repository.PaymentLogic
import com.warrenbrowse.vpn.lib.repository.UserPreferencesRepository
import com.warrenbrowse.vpn.lib.usecase.LastKnownLocationUseCase
import com.warrenbrowse.vpn.lib.usecase.OutOfTimeUseCase
import com.warrenbrowse.vpn.lib.usecase.SelectedLocationTitleUseCase
import com.warrenbrowse.vpn.lib.usecase.SystemVpnSettingsAvailableUseCase

@Suppress("LongParameterList")
class ConnectViewModel(
    private val accountRepository: AccountRepository,
    private val deviceRepository: DeviceRepository,
    private val changelogRepository: ChangelogRepository,
    inAppNotificationController: InAppNotificationController,
    private val newDeviceRepository: NewDeviceRepository,
    private val userPreferencesRepository: UserPreferencesRepository,
    selectedLocationTitleUseCase: SelectedLocationTitleUseCase,
    private val outOfTimeUseCase: OutOfTimeUseCase,
    private val paymentUseCase: PaymentLogic,
    private val connectionProxy: ConnectionProxy,
    lastKnownLocationUseCase: LastKnownLocationUseCase,
    private val systemVpnSettingsUseCase: SystemVpnSettingsAvailableUseCase,
    private val isPlayBuild: Boolean,
    private val resolveAppListing: ResolveAppListingUseCase,
) : ViewModel() {
    private val _uiSideEffect = Channel<UiSideEffect>()

    val uiSideEffect =
        merge(_uiSideEffect.receiveAsFlow(), outOfTimeEffect(), revokedDeviceEffect())

    @OptIn(FlowPreview::class)
    val uiState: StateFlow<ConnectUiState> =
        combine(
                selectedLocationTitleUseCase(),
                inAppNotificationController.notifications,
                connectionProxy.tunnelState.withPrev(),
                lastKnownLocationUseCase.lastKnownDisconnectedLocation,
                accountRepository.accountData,
                deviceRepository.deviceState.map { it?.displayName() },
            ) {
                selectedRelayItemTitle,
                notifications,
                (tunnelState, prevTunnelState),
                lastKnownDisconnectedLocation,
                accountData,
                deviceName ->
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
                    deviceName = deviceName,
                    daysLeftUntilExpiry = accountData?.expiryDate?.daysLeft(),
                    isPlayBuild = isPlayBuild,
                )
            }
            .onStart {
                viewModelScope.launch {
                    accountRepository.refreshAccountData(ignoreTimeout = false)
                }
            }
            .stateIn(
                viewModelScope,
                SharingStarted.WhileSubscribed(VIEW_MODEL_STOP_TIMEOUT),
                ConnectUiState.INITIAL,
            )

    init {
        viewModelScope.launch {
            if (paymentUseCase.verifyPurchases().getOrNull() == VerificationResult.Success) {
                accountRepository.refreshAccountData()
            }
        }
        viewModelScope.launch { deviceRepository.updateDevice() }
    }

    fun onDisconnectClick() {
        viewModelScope.launch {
            connectionProxy.disconnect(DisconnectReason.USER_INITIATED_DISCONNECT_BUTTON).onLeft {
                _uiSideEffect.send(UiSideEffect.ConnectError.Generic)
            }
        }
    }

    fun onReconnectClick() {
        viewModelScope.launch {
            connectionProxy.reconnect().onLeft {
                _uiSideEffect.send(UiSideEffect.ConnectError.Generic)
            }
        }
    }

    fun onConnectClick() {
        viewModelScope.launch {
            connectionProxy.connect().onLeft { connectError ->
                when (connectError) {
                    is ConnectError.Unknown -> _uiSideEffect.send(UiSideEffect.ConnectError.Generic)
                    is ConnectError.NotPrepared ->
                        _uiSideEffect.send(UiSideEffect.NotPrepared(connectError.error))
                }
            }
        }
    }

    fun createVpnProfileResult(hasVpnPermission: Boolean) {
        viewModelScope.launch {
            if (hasVpnPermission) {
                connectionProxy.connect()
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
            connectionProxy.disconnect(DisconnectReason.USER_INITIATED_CANCEL_BUTTON).onLeft {
                _uiSideEffect.send(UiSideEffect.ConnectError.Generic)
            }
        }
    }

    fun onManageAccountClick() {
        viewModelScope.launch {
            val wwwAuthToken = accountRepository.getWebsiteAuthToken()
            _uiSideEffect.send(UiSideEffect.OpenAccountManagementPageInBrowser(wwwAuthToken))
        }
    }

    fun openAppListing() = viewModelScope.launch {
        val target = resolveAppListing()
        val sideEffect =
            UiSideEffect.OpenUri(
                uri = target.listingUri.toUri(),
                errorMessage = target.errorMessage,
            )
        _uiSideEffect.send(sideEffect)
    }

    fun dismissNewDeviceNotification() {
        newDeviceRepository.clearNewDeviceCreatedNotification()
    }

    fun dismissAndroid16UpgradeWarning() = viewModelScope.launch {
        userPreferencesRepository.setShowAndroid16ConnectWarning(false)
    }

    fun dismissNewChangelogNotification() = viewModelScope.launch {
        changelogRepository.setDismissNewChangelogNotification()
    }

    private fun outOfTimeEffect() =
        outOfTimeUseCase.isOutOfTime.filter { it == true }.map { UiSideEffect.OutOfTime }

    private fun revokedDeviceEffect() =
        deviceRepository.deviceState.filterIsInstance<DeviceState.Revoked>().map {
            UiSideEffect.RevokedDevice
        }

    sealed interface UiSideEffect {
        data class OpenAccountManagementPageInBrowser(val token: WebsiteAuthToken?) : UiSideEffect

        data object OutOfTime : UiSideEffect

        data class OpenUri(val uri: Uri, val errorMessage: String) : UiSideEffect

        data object RevokedDevice : UiSideEffect

        data class NotPrepared(val prepareError: PrepareError) : UiSideEffect

        sealed interface ConnectError : UiSideEffect {
            data object Generic : ConnectError

            data class PermissionDenied(val systemVpnSettingsAvailable: Boolean) : ConnectError
        }
    }
}
