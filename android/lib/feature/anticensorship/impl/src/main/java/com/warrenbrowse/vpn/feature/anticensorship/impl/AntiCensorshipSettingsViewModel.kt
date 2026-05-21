package com.warrenbrowse.vpn.feature.anticensorship.impl

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.WhileSubscribed
import kotlinx.coroutines.flow.filterNotNull
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import com.warrenbrowse.vpn.lib.common.Lc
import com.warrenbrowse.vpn.lib.common.constant.VIEW_MODEL_STOP_TIMEOUT
import com.warrenbrowse.vpn.lib.common.toLc
import com.warrenbrowse.vpn.lib.common.util.selectedObfuscationMode
import com.warrenbrowse.vpn.lib.common.util.wireguardPort
import com.warrenbrowse.vpn.lib.model.ObfuscationMode
import com.warrenbrowse.vpn.lib.repository.SettingsRepository

sealed interface AntiCensorshipSideEffect {
    sealed interface ShowToast : AntiCensorshipSideEffect {
        data object GenericError : ShowToast
    }
}

class AntiCensorshipSettingsViewModel(
    private val isModal: Boolean,
    private val settingsRepository: SettingsRepository,
    private val ioDispatcher: CoroutineDispatcher = Dispatchers.IO,
) : ViewModel() {

    private val _uiSideEffect = Channel<AntiCensorshipSideEffect>()
    val uiSideEffect = _uiSideEffect.receiveAsFlow()

    val uiState: StateFlow<Lc<Unit, AntiCensorshipSettingsUiState>> =
        settingsRepository.settingsUpdates
            .filterNotNull()
            .map { settings ->
                AntiCensorshipSettingsUiState.from(
                        isModal = isModal,
                        obfuscationMode = settings.selectedObfuscationMode(),
                        selectedUdp2TcpObfuscationPort = settings.obfuscationSettings.udp2tcp.port,
                        selectedShadowsocksObfuscationPort =
                            settings.obfuscationSettings.shadowsocks.port,
                        selectedWireguardPort = settings.wireguardPort(),
                    )
                    .toLc<Unit, AntiCensorshipSettingsUiState>()
            }
            .stateIn(
                viewModelScope,
                SharingStarted.WhileSubscribed(VIEW_MODEL_STOP_TIMEOUT),
                Lc.Loading(Unit),
            )

    fun onSelectObfuscationMode(obfuscationMode: ObfuscationMode) {
        viewModelScope.launch(ioDispatcher) {
            settingsRepository.setObfuscation(obfuscationMode).onLeft {
                _uiSideEffect.send(AntiCensorshipSideEffect.ShowToast.GenericError)
            }
        }
    }
}
