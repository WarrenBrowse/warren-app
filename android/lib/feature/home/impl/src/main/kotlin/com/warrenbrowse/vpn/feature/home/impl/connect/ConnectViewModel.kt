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
import com.warrenbrowse.vpn.lib.model.GeoIpLocation
import com.warrenbrowse.vpn.lib.model.PrepareError
import com.warrenbrowse.vpn.lib.model.TunnelState
import com.warrenbrowse.vpn.lib.repository.ChangelogRepository
import com.warrenbrowse.vpn.lib.repository.ConnectionProxy
import com.warrenbrowse.vpn.lib.repository.DeviceRepository
import com.warrenbrowse.vpn.lib.repository.UserPreferencesRepository
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository
import com.warrenbrowse.vpn.lib.repository.WarrenAutoRecoveryProvider
import com.warrenbrowse.vpn.lib.repository.WarrenHostOfflineProvider
import com.warrenbrowse.vpn.lib.repository.WarrenQuinnDisconnectInvoker
import com.warrenbrowse.vpn.lib.repository.WarrenQuinnReconnectInvoker
import com.warrenbrowse.vpn.lib.repository.WarrenRelayProvider
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
    private val relayProvider: WarrenRelayProvider,
    private val localSettings: WarrenLocalSettingsRepository,
    hostOfflineProvider: WarrenHostOfflineProvider,
    autoRecoveryProvider: WarrenAutoRecoveryProvider,
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
                localSettings.selectedExitId,
                hostOfflineProvider.hostOffline,
                autoRecoveryProvider.autoRecoveryCount,
            ) {
                selectedRelayItemTitle,
                notifications,
                (tunnelState, prevTunnelState),
                lastKnownDisconnectedLocation,
                selectedExitId,
                hostOffline,
                autoRecoveryCount ->
                // Warren's relay list carries no coordinates and there is no
                // device-GeoIP service, so the Warren tunnel state never reports
                // a location. Derive one from the selected (or first active) exit
                // relay's country centroid so the map plots a marker in both
                // states, mirroring the desktop (which also falls back to the
                // selected relay while disconnected).
                val relayLocation = selectedExitLocation(selectedExitId)
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
                        } ?: relayLocation,
                    selectedRelayItemTitle =
                        if (tunnelState is TunnelState.Disconnected) {
                            selectedRelayItemTitle
                        } else {
                            null
                        },
                    tunnelState = tunnelState,
                    inAppNotification = notifications.firstOrNull(),
                    isPlayBuild = isPlayBuild,
                    hostOffline = hostOffline,
                    autoRecoveryCount = autoRecoveryCount,
                )
            }
            .stateIn(
                viewModelScope,
                SharingStarted.WhileSubscribed(VIEW_MODEL_STOP_TIMEOUT),
                ConnectUiState.INITIAL,
            )

    init {
        viewModelScope.launch { deviceRepository.updateDevice() }
    }

    /**
     * Build a [GeoIpLocation] for the map marker from the selected exit relay
     * (or the first active relay when none is pinned, i.e. "Automatic"), using
     * the country centroid since the relay list carries no coordinates. Returns
     * null when the catalogue is empty or the country has no centroid.
     */
    private fun selectedExitLocation(selectedExitId: String?): GeoIpLocation? {
        val relays = relayProvider.list()
        val exit = relays.firstOrNull { it.exitId == selectedExitId }
            ?: relays.firstOrNull { it.active }
            ?: return null
        val (lat, lon) = CountryCentroid.of(exit.country) ?: return null
        return GeoIpLocation(
            ipv4 = null,
            ipv6 = null,
            country = exit.country,
            city = exit.city.ifBlank { null },
            latitude = lat,
            longitude = lon,
            hostname = null,
            entryHostname = null,
        )
    }

    fun onDisconnectClick() {
        viewModelScope.launch {
            // Route disconnect through WarrenQuinnDisconnectInvoker for the
            // real Warren tunnel teardown.
            warrenDisconnect.disconnect()
        }
    }

    fun onReconnectClick() {
        viewModelScope.launch {
            warrenReconnect.reconnect()
        }
    }

    fun onConnectClick() {
        // Connect is dispatched from `ConnectScreen` directly through
        // `WarrenQuinnConnectInvoker.connect(activity)` since the invoker
        // needs a FragmentActivity for biometric unlock; the ViewModel only
        // emits a side effect requesting the screen to perform the dispatch.
        viewModelScope.launch {
            _uiSideEffect.send(UiSideEffect.RequestWarrenConnect)
        }
    }

    fun createVpnProfileResult(hasVpnPermission: Boolean) {
        viewModelScope.launch {
            if (hasVpnPermission) {
                // VPN permission granted: request the UI to dispatch
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
            // Warren has no separate Cancel command; cancelling an
            // in-progress connect tears down the in-flight tunnel.
            warrenDisconnect.disconnect()
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

        // Connect dispatch needs a FragmentActivity for biometric unlock, so
        // the VM emits this side effect and ConnectScreen invokes
        // WarrenQuinnConnectInvoker.connect(activity) on the current Activity.
        data object RequestWarrenConnect : UiSideEffect

        data class NotPrepared(val prepareError: PrepareError) : UiSideEffect

        sealed interface ConnectError : UiSideEffect {
            data object Generic : ConnectError

            data class PermissionDenied(val systemVpnSettingsAvailable: Boolean) : ConnectError
        }
    }
}
