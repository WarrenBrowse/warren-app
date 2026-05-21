package com.warrenbrowse.vpn.feature.anticensorship.impl.selectport

import android.content.res.Resources
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import arrow.core.Either
import arrow.core.right
import co.touchlab.kermit.Logger
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.WhileSubscribed
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.filterNotNull
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import com.warrenbrowse.vpn.feature.anticensorship.api.SelectPortNavKey
import com.warrenbrowse.vpn.feature.anticensorship.impl.SHADOWSOCKS_AVAILABLE_PORTS
import com.warrenbrowse.vpn.feature.anticensorship.impl.SHADOWSOCKS_PRESET_PORTS
import com.warrenbrowse.vpn.feature.anticensorship.impl.UDP2TCP_PRESET_PORTS
import com.warrenbrowse.vpn.feature.anticensorship.impl.WIREGUARD_PRESET_PORTS
import com.warrenbrowse.vpn.lib.common.Lc
import com.warrenbrowse.vpn.lib.common.constant.VIEW_MODEL_STOP_TIMEOUT
import com.warrenbrowse.vpn.lib.common.toLc
import com.warrenbrowse.vpn.lib.model.Constraint
import com.warrenbrowse.vpn.lib.model.ObfuscationSettings
import com.warrenbrowse.vpn.lib.model.Port
import com.warrenbrowse.vpn.lib.model.PortRange
import com.warrenbrowse.vpn.lib.model.PortType
import com.warrenbrowse.vpn.lib.model.SetObfuscationOptionsError
import com.warrenbrowse.vpn.lib.repository.RelayListRepository
import com.warrenbrowse.vpn.lib.repository.SettingsRepository
import com.warrenbrowse.vpn.lib.ui.resource.R

class SelectPortViewModel(
    navArgs: SelectPortNavKey,
    private val settingsRepository: SettingsRepository,
    private val resources: Resources,
    relayListRepository: RelayListRepository,
) : ViewModel() {

    private val portType = navArgs.portType

    private val initialOrCustomPort = MutableStateFlow<Port?>(null)

    init {
        viewModelScope.launch {
            val initialSettings = settingsRepository.settingsUpdates.filterNotNull().first()
            initialOrCustomPort.value =
                initialSettings.obfuscationSettings.port(portType).getOrNull()
        }
    }

    val uiState: StateFlow<Lc<Unit, SelectPortUiState>> =
        combine(
                settingsRepository.settingsUpdates.filterNotNull(),
                relayListRepository.portRanges,
                relayListRepository.shadowsocksPortRanges,
                initialOrCustomPort,
            ) { settings, wireguardPortRanges, shadowsocksPortRanges, initialOrCustomPort ->
                val portTypeState =
                    portType.uiState(
                        wireguardPortRanges = wireguardPortRanges,
                        shadowsocksPortRanges = shadowsocksPortRanges,
                    )
                val customPort =
                    if (initialOrCustomPort !in portTypeState.presetPorts) initialOrCustomPort
                    else null

                SelectPortUiState(
                        portType = portType,
                        port = settings.obfuscationSettings.port(portType),
                        presetPorts = portTypeState.presetPorts,
                        customPortEnabled = portTypeState.customPortEnabled,
                        title = portTypeState.title,
                        allowedPortRanges = portTypeState.allowedPortRanges,
                        recommendedPortRanges = portTypeState.recommendedPortRanges,
                        customPort = customPort,
                    )
                    .toLc<Unit, SelectPortUiState>()
            }
            .stateIn(
                scope = viewModelScope,
                started = SharingStarted.WhileSubscribed(VIEW_MODEL_STOP_TIMEOUT),
                initialValue = Lc.Loading(Unit),
            )

    fun onPortSelected(port: Constraint<Port>) {
        viewModelScope.launch {
            updatePort(port)
                .onLeft { Logger.e("Select shadowsocks port error $it") }
                .onRight {
                    val presets = uiState.value.contentOrNull()?.presetPorts ?: emptyList()
                    if (port is Constraint.Only && port.value !in presets) {
                        initialOrCustomPort.update { port.getOrNull() }
                    }
                }
        }
    }

    private suspend fun updatePort(
        port: Constraint<Port>
    ): Either<SetObfuscationOptionsError, Unit> =
        when (portType) {
            PortType.Udp2Tcp -> settingsRepository.setCustomUdp2TcpObfuscationPort(port)
            PortType.Shadowsocks -> settingsRepository.setCustomShadowsocksObfuscationPort(port)
            PortType.Wireguard -> settingsRepository.setCustomWireguardPort(port)
            PortType.Lwo -> Unit.right()
        }

    fun resetCustomPort() {
        val isCustom = uiState.value.contentOrNull()?.isCustom == true
        initialOrCustomPort.update { null }
        // If custom port was selected, update selection to be any.
        if (isCustom) {
            viewModelScope.launch { updatePort(Constraint.Any) }
        }
    }

    private fun PortType.uiState(
        wireguardPortRanges: List<PortRange>,
        shadowsocksPortRanges: List<PortRange>,
    ): PortTypeUiState =
        when (this) {
            PortType.Udp2Tcp ->
                PortTypeUiState(
                    presetPorts = UDP2TCP_PRESET_PORTS,
                    allowedPortRanges = emptyList(),
                    recommendedPortRanges = emptyList(),
                    customPortEnabled = false,
                    title = resources.getString(R.string.udp_over_tcp),
                )
            PortType.Shadowsocks ->
                PortTypeUiState(
                    presetPorts = SHADOWSOCKS_PRESET_PORTS,
                    allowedPortRanges = SHADOWSOCKS_AVAILABLE_PORTS,
                    recommendedPortRanges = shadowsocksPortRanges,
                    customPortEnabled = true,
                    title = resources.getString(R.string.shadowsocks),
                )
            PortType.Wireguard ->
                PortTypeUiState(
                    presetPorts = WIREGUARD_PRESET_PORTS,
                    allowedPortRanges = wireguardPortRanges,
                    recommendedPortRanges = emptyList(),
                    customPortEnabled = true,
                    title = resources.getString(R.string.wireguard_port_title),
                )
            PortType.Lwo ->
                PortTypeUiState(
                    presetPorts = emptyList(),
                    allowedPortRanges = emptyList(),
                    recommendedPortRanges = emptyList(),
                    customPortEnabled = false,
                    title = resources.getString(R.string.lwo),
                )
        }

    private fun ObfuscationSettings.port(portType: PortType): Constraint<Port> =
        when (portType) {
            PortType.Udp2Tcp -> udp2tcp.port
            PortType.Shadowsocks -> shadowsocks.port
            PortType.Wireguard -> wireguardPort
            PortType.Lwo -> Constraint.Any
        }
}
