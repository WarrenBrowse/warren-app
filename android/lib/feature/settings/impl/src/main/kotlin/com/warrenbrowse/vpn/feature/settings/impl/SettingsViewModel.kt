package com.warrenbrowse.vpn.feature.settings.impl

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.WhileSubscribed
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.stateIn
import com.warrenbrowse.vpn.lib.common.Lc
import com.warrenbrowse.vpn.lib.common.constant.VIEW_MODEL_STOP_TIMEOUT
import com.warrenbrowse.vpn.lib.common.toLc
import com.warrenbrowse.vpn.lib.model.wallet.WalletState
import com.warrenbrowse.vpn.lib.repository.AppVersionInfoRepository
import com.warrenbrowse.vpn.lib.repository.WalletRepository
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository

class SettingsViewModel(
    walletRepository: WalletRepository,
    warrenLocalSettings: WarrenLocalSettingsRepository,
    appVersionInfoRepository: AppVersionInfoRepository,
    isPlayBuild: Boolean,
) : ViewModel() {

    val uiState: StateFlow<Lc<Unit, SettingsUiState>> =
        combine(
                walletRepository.state,
                warrenLocalSettings.daitaEnabled,
                appVersionInfoRepository.versionInfo,
            ) { walletState, daita, versionInfo ->
                SettingsUiState(
                        isLoggedIn = walletState is WalletState.Ready,
                        appVersion = versionInfo.currentVersion,
                        isSupportedVersion = versionInfo.isSupported,
                        isDaitaEnabled = daita,
                        isPlayBuild = isPlayBuild,
                    )
                    .toLc<Unit, SettingsUiState>()
            }
            .stateIn(
                viewModelScope,
                SharingStarted.WhileSubscribed(VIEW_MODEL_STOP_TIMEOUT),
                Lc.Loading(Unit),
            )
}
