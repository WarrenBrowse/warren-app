package com.warrenbrowse.vpn.feature.splittunneling.impl

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.WhileSubscribed
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import com.warrenbrowse.vpn.feature.splittunneling.impl.applist.SplitTunnelingUseCase
import com.warrenbrowse.vpn.lib.common.Lc
import com.warrenbrowse.vpn.lib.common.constant.VIEW_MODEL_STOP_TIMEOUT
import com.warrenbrowse.vpn.lib.model.PackageName
import com.warrenbrowse.vpn.lib.repository.SplitTunnelingRepository
import com.warrenbrowse.vpn.lib.repository.UserPreferencesRepository

class SplitTunnelingViewModel(
    isModal: Boolean,
    private val splitTunnelingRepository: SplitTunnelingRepository,
    private val userPreferencesRepository: UserPreferencesRepository,
    splitTunnelingUseCase: SplitTunnelingUseCase,
    private val dispatcher: CoroutineDispatcher,
) : ViewModel() {

    val uiState: StateFlow<Lc<Loading, SplitTunnelingUiState>> =
        combine(
                splitTunnelingUseCase(),
                splitTunnelingRepository.splitTunnelingEnabled,
                userPreferencesRepository.showSystemAppsSplitTunneling(),
            ) { splitApps, enabled, showSystemApps ->
                Lc.Content(
                    SplitTunnelingUiState(
                        enabled = enabled,
                        excludedApps = splitApps.excludedApps,
                        includedApps = splitApps.includedApps,
                        showSystemApps = showSystemApps,
                        isModal = isModal,
                    )
                )
            }
            .stateIn(
                viewModelScope,
                SharingStarted.WhileSubscribed(VIEW_MODEL_STOP_TIMEOUT),
                Lc.Loading(Loading(isModal = isModal)),
            )

    fun onEnableSplitTunneling(isEnabled: Boolean) {
        viewModelScope.launch(dispatcher) {
            splitTunnelingRepository.enableSplitTunneling(isEnabled)
        }
    }

    fun onIncludeAppClick(packageName: PackageName) {
        viewModelScope.launch(dispatcher) { splitTunnelingRepository.includeApp(packageName) }
    }

    fun onExcludeAppClick(packageName: PackageName) {
        viewModelScope.launch(dispatcher) { splitTunnelingRepository.excludeApp(packageName) }
    }

    fun onShowSystemAppsClick(show: Boolean) {
        viewModelScope.launch(dispatcher) {
            userPreferencesRepository.setShowSystemAppsSplitTunneling(show)
        }
    }
}
