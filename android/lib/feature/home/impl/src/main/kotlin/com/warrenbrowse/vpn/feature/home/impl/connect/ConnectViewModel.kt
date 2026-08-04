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
import kotlinx.coroutines.flow.combine
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
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository
import com.warrenbrowse.vpn.lib.repository.WarrenAutoRecoveryProvider
import com.warrenbrowse.vpn.lib.repository.WarrenHostOfflineProvider
import com.warrenbrowse.vpn.lib.repository.WarrenQuinnDisconnectInvoker
import com.warrenbrowse.vpn.lib.repository.WarrenRelayProvider
import com.warrenbrowse.vpn.lib.repository.WarrenPathHealthProvider
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
    private val isPlayBuild: Boolean,
    private val resolveAppListing: ResolveAppListingUseCase,
    private val relayProvider: WarrenRelayProvider,
    pathHealthProvider: WarrenPathHealthProvider,
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
                localSettings.exitPin,
                // A wedged datapath is "connected but carrying nothing", which
                // reads to the user exactly like being offline, so it feeds the
                // same honesty surface: the card drops "Connection established"
                // for "Connection interrupted" instead of claiming protection.
                hostOfflineProvider.hostOffline.combine(pathHealthProvider.pathWedged) {
                    offline,
                    wedged ->
                    offline || wedged
                },
                autoRecoveryProvider.autoRecoveryCount,
                // The pinned location below is derived from the relay
                // catalogue, which is fetched asynchronously. Without this
                // input the first pass latches the empty cold-cache snapshot
                // and the location stays null until another input happens to
                // change.
                relayProvider.catalogue,
            ) {
                selectedRelayItemTitle,
                notifications,
                (tunnelState, prevTunnelState),
                lastKnownDisconnectedLocation,
                exitPin,
                hostOffline,
                autoRecoveryCount,
                relays ->
                // Warren's relay list carries no coordinates and there is no
                // device-GeoIP service, so the Warren tunnel state never reports
                // a location. The pinned scope stands in when the engine has not
                // named an exit yet, and it is null under Automatic: naming an
                // arbitrary catalogue relay would paint the wrong country until
                // the tunnel came up (desktop returns no target country for an
                // "any" constraint).
                val pinnedLocation = pinnedExitLocation(exitPin, relays)
                ConnectUiState(
                    location =
                        when (tunnelState) {
                            is TunnelState.Disconnected ->
                                tunnelState.location ?: lastKnownDisconnectedLocation

                            // The dialled exit is known from the first connecting
                            // frame, so resolve it the same way as a live tunnel
                            // instead of guessing.
                            is TunnelState.Connecting ->
                                tunnelState.location
                                    ?: tunnelState.endpoint?.let {
                                        activeExitLocation(it.endpoint, relays)
                                    }
                            // Key the title off the ACTIVE exit (the one the
                            // tunnel actually egresses through), not the pinned
                            // one: after switching exit without a reconnect the
                            // pin already points at the new exit while traffic
                            // still runs the old one, so falling back to the
                            // pinned location here would mislabel a live tunnel.
                            // Matched by endpoint host against the relay
                            // catalogue.
                            is TunnelState.Connected ->
                                tunnelState.location
                                    ?: activeExitLocation(tunnelState.endpoint.endpoint, relays)
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
                        } ?: pinnedLocation,
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

    fun onDisconnectClick() {
        viewModelScope.launch {
            // Route disconnect through WarrenQuinnDisconnectInvoker for the
            // real Warren tunnel teardown.
            warrenDisconnect.disconnect()
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

    /**
     * Records the version so the prompt stays down for THIS upgrade only; the
     * next release raises it again.
     */
    fun dismissUpdateAvailable(version: String) = viewModelScope.launch {
        userPreferencesRepository.setDismissedUpgradeVersion(version)
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
