package com.warrenbrowse.vpn.feature.vpnsettings.impl

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import arrow.core.None
import arrow.core.Option
import arrow.core.Some
import co.touchlab.kermit.Logger
import java.net.Inet6Address
import java.net.InetAddress
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.WhileSubscribed
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.filterIsInstance
import kotlinx.coroutines.flow.filterNotNull
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import com.warrenbrowse.vpn.feature.vpnsettings.api.VpnSettingsNavKey
import com.warrenbrowse.vpn.lib.common.Lc
import com.warrenbrowse.vpn.lib.common.constant.VIEW_MODEL_STOP_TIMEOUT
import com.warrenbrowse.vpn.lib.common.toLc
import com.warrenbrowse.vpn.lib.common.util.contentBlockersSettings
import com.warrenbrowse.vpn.lib.common.util.customDnsAddresses
import com.warrenbrowse.vpn.lib.common.util.deviceIpVersion
import com.warrenbrowse.vpn.lib.common.util.isCustomDnsEnabled
import com.warrenbrowse.vpn.lib.common.util.onFirst
import com.warrenbrowse.vpn.lib.common.util.quantumResistant
import com.warrenbrowse.vpn.lib.common.util.selectedObfuscationMode
import com.warrenbrowse.vpn.lib.model.Constraint
import com.warrenbrowse.vpn.lib.model.DefaultDnsOptions
import com.warrenbrowse.vpn.lib.model.DnsState
import com.warrenbrowse.vpn.lib.model.FeatureIndicator
import com.warrenbrowse.vpn.lib.model.IpVersion
import com.warrenbrowse.vpn.lib.model.QuantumResistantState
import com.warrenbrowse.vpn.lib.repository.AutoStartAndConnectOnBootRepository
import com.warrenbrowse.vpn.lib.repository.SettingsRepository
import com.warrenbrowse.vpn.lib.repository.WireguardConstraintsRepository
import com.warrenbrowse.vpn.lib.usecase.SystemVpnSettingsAvailableUseCase

sealed interface VpnSettingsSideEffect {
    sealed interface ShowToast : VpnSettingsSideEffect {
        data object ApplySettingsWarning : ShowToast

        data object GenericError : ShowToast
    }

    data object NavigateToDnsDialog : VpnSettingsSideEffect
}

@Suppress("TooManyFunctions")
class VpnSettingsViewModel(
    private val navArgs: VpnSettingsNavKey,
    private val settingsRepository: SettingsRepository,
    private val systemVpnSettingsUseCase: SystemVpnSettingsAvailableUseCase,
    private val autoStartAndConnectOnBootRepository: AutoStartAndConnectOnBootRepository,
    private val wireguardConstraintsRepository: WireguardConstraintsRepository,
    private val dispatcher: CoroutineDispatcher = Dispatchers.IO,
) : ViewModel() {
    private val _mutableIsContentBlockersExpanded = MutableStateFlow<Option<Boolean>>(None)

    private val _uiSideEffect = Channel<VpnSettingsSideEffect>()
    val uiSideEffect = _uiSideEffect.receiveAsFlow()
    val uiState =
        combine(
                settingsRepository.settingsUpdates.filterNotNull().onFirst {
                    // If we are coming from the dns content blockers feature indicator we should
                    // expand the content blockers section.
                    _mutableIsContentBlockersExpanded.value =
                        Some(navArgs.scrollToFeature == FeatureIndicator.DNS_CONTENT_BLOCKERS)
                },
                autoStartAndConnectOnBootRepository.autoStartAndConnectOnBoot,
                _mutableIsContentBlockersExpanded.filterIsInstance<Some<Boolean>>().map { it.value },
            ) { settings, autoStartAndConnectOnBoot, isContentBlockersExpanded ->
                VpnSettingsUiState.from(
                        mtu = settings.tunnelOptions.mtu,
                        isLocalNetworkSharingEnabled = settings.allowLan,
                        isCustomDnsEnabled = settings.isCustomDnsEnabled(),
                        customDnsItems = settings.customDnsAddresses().asStringAddressList(),
                        contentBlockersOptions = settings.contentBlockersSettings(),
                        obfuscationMode = settings.selectedObfuscationMode(),
                        quantumResistant = settings.quantumResistant(),
                        systemVpnSettingsAvailable = systemVpnSettingsUseCase(),
                        autoStartAndConnectOnBoot = autoStartAndConnectOnBoot,
                        deviceIpVersion = settings.deviceIpVersion(),
                        isIpv6Enabled = settings.tunnelOptions.enableIpv6,
                        isContentBlockersExpanded = isContentBlockersExpanded,
                        isModal = navArgs.isModal,
                    )
                    .toLc<Boolean, VpnSettingsUiState>()
            }
            .stateIn(
                viewModelScope,
                SharingStarted.WhileSubscribed(VIEW_MODEL_STOP_TIMEOUT),
                Lc.Loading(navArgs.isModal),
            )

    fun onToggleLocalNetworkSharing(isEnabled: Boolean) {
        viewModelScope.launch(dispatcher) {
            settingsRepository.setLocalNetworkSharing(isEnabled).onLeft {
                _uiSideEffect.send(VpnSettingsSideEffect.ShowToast.GenericError)
            }
        }
    }

    fun onToggleCustomDns(enable: Boolean) = viewModelScope.launch {
        val settings = settingsRepository.settingsUpdates.value
        if (settings == null) {
            showGenericErrorToast()
            return@launch
        }

        val hasDnsEntries = settings.customDnsAddresses().isNotEmpty()

        if (hasDnsEntries) {
            settingsRepository
                .setDnsState(if (enable) DnsState.Custom else DnsState.Default)
                .fold({ showGenericErrorToast() }, { showApplySettingChangesWarningToast() })
        } else {
            // If they enable custom DNS and has no current entries we show the dialog
            // to add one.
            viewModelScope.launch { _uiSideEffect.send(VpnSettingsSideEffect.NavigateToDnsDialog) }
        }
    }

    fun onToggleContentBlockersExpand() = _mutableIsContentBlockersExpanded.update {
        it.map { expand -> !expand }
    }

    fun onToggleAllBlockers(isEnabled: Boolean) = updateContentBlockersAndNotify {
        DefaultDnsOptions(
            blockAds = isEnabled,
            blockTrackers = isEnabled,
            blockMalware = isEnabled,
            blockAdultContent = isEnabled,
            blockGambling = isEnabled,
            blockSocialMedia = isEnabled,
        )
    }

    fun onToggleBlockAds(isEnabled: Boolean) = updateContentBlockersAndNotify {
        it.copy(blockAds = isEnabled)
    }

    fun onToggleBlockTrackers(isEnabled: Boolean) = updateContentBlockersAndNotify {
        it.copy(blockTrackers = isEnabled)
    }

    fun onToggleBlockMalware(isEnabled: Boolean) = updateContentBlockersAndNotify {
        it.copy(blockMalware = isEnabled)
    }

    fun onToggleBlockAdultContent(isEnabled: Boolean) = updateContentBlockersAndNotify {
        it.copy(blockAdultContent = isEnabled)
    }

    fun onToggleBlockGambling(isEnabled: Boolean) = updateContentBlockersAndNotify {
        it.copy(blockGambling = isEnabled)
    }

    fun onToggleBlockSocialMedia(isEnabled: Boolean) = updateContentBlockersAndNotify {
        it.copy(blockSocialMedia = isEnabled)
    }

    fun onSelectQuantumResistanceSetting(enable: Boolean) {
        viewModelScope.launch(dispatcher) {
            settingsRepository
                .setWireguardQuantumResistant(
                    if (enable) {
                        QuantumResistantState.On
                    } else {
                        QuantumResistantState.Off
                    }
                )
                .onLeft { _uiSideEffect.send(VpnSettingsSideEffect.ShowToast.GenericError) }
        }
    }

    fun onToggleAutoStartAndConnectOnBoot(autoStartAndConnect: Boolean) =
        viewModelScope.launch(dispatcher) {
            autoStartAndConnectOnBootRepository.setAutoStartAndConnectOnBoot(autoStartAndConnect)
        }

    fun onDeviceIpVersionSelected(ipVersion: Constraint<IpVersion>) =
        viewModelScope.launch(dispatcher) {
            wireguardConstraintsRepository.setDeviceIpVersion(ipVersion).onLeft {
                _uiSideEffect.send(VpnSettingsSideEffect.ShowToast.GenericError)
            }
        }

    fun setIpv6Enabled(enable: Boolean) =
        viewModelScope.launch(dispatcher) {
            settingsRepository.setIpv6Enabled(enable).onLeft {
                _uiSideEffect.send(VpnSettingsSideEffect.ShowToast.GenericError)
            }
        }

    private fun updateContentBlockersAndNotify(update: (DefaultDnsOptions) -> DefaultDnsOptions) =
        viewModelScope.launch(dispatcher) {
            settingsRepository
                .updateContentBlockers(update)
                .fold(
                    {
                        Logger.e("Failed to update content blockers")
                        _uiSideEffect.send(VpnSettingsSideEffect.ShowToast.GenericError)
                    },
                    { showApplySettingChangesWarningToast() },
                )
        }

    private fun List<InetAddress>.asStringAddressList(): List<CustomDnsItem> = map {
        CustomDnsItem(
            address = it.hostAddress ?: EMPTY_STRING,
            isLocal = it.isLocalAddress(),
            isIpv6 = it is Inet6Address,
        )
    }

    private fun InetAddress.isLocalAddress(): Boolean = isLinkLocalAddress || isSiteLocalAddress

    fun showApplySettingChangesWarningToast() = viewModelScope.launch {
        _uiSideEffect.send(VpnSettingsSideEffect.ShowToast.ApplySettingsWarning)
    }

    fun showGenericErrorToast() = viewModelScope.launch {
        _uiSideEffect.send(VpnSettingsSideEffect.ShowToast.GenericError)
    }

    companion object {
        private const val EMPTY_STRING = ""
    }
}
