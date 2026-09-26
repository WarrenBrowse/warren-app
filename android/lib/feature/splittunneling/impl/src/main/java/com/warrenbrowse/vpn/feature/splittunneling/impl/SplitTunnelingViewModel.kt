package com.warrenbrowse.vpn.feature.splittunneling.impl

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.flow.MutableStateFlow
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
import com.warrenbrowse.vpn.lib.model.SplitTunnelMode
import com.warrenbrowse.vpn.lib.repository.SplitTunnelingRepository
import com.warrenbrowse.vpn.lib.repository.UserPreferencesRepository

class SplitTunnelingViewModel(
    isModal: Boolean,
    private val splitTunnelingRepository: SplitTunnelingRepository,
    private val userPreferencesRepository: UserPreferencesRepository,
    splitTunnelingUseCase: SplitTunnelingUseCase,
    private val dispatcher: CoroutineDispatcher,
) : ViewModel() {

    // The screen opens on the tab of the mode in force, so a user sent here by
    // the "VPN only for" label lands on its list.
    private val tab =
        MutableStateFlow(
            if (splitTunnelingRepository.splitMode.value == SplitTunnelMode.IncludeOnly) {
                SplitTunnelingTab.IncludeOnly
            } else {
                SplitTunnelingTab.Bypass
            }
        )

    private val pendingMode = MutableStateFlow<SplitTunnelMode?>(null)

    val uiState: StateFlow<Lc<Loading, SplitTunnelingUiState>> =
        combine(
                splitTunnelingUseCase(tab),
                splitTunnelingRepository.splitMode,
                userPreferencesRepository.showSystemAppsSplitTunneling(),
                tab,
                pendingMode,
            ) { splitApps, mode, showSystemApps, shownTab, pending ->
                Lc.Content(
                    SplitTunnelingUiState(
                        splitMode = mode,
                        tab = shownTab,
                        selectedApps = splitApps.selectedApps,
                        otherApps = splitApps.otherApps,
                        showSystemApps = showSystemApps,
                        isModal = isModal,
                        confirmation = pending?.let { modeChangeConfirmation(mode, it) },
                    )
                )
            }
            .stateIn(
                viewModelScope,
                SharingStarted.WhileSubscribed(VIEW_MODEL_STOP_TIMEOUT),
                Lc.Loading(Loading(isModal = isModal)),
            )

    fun onSelectTab(selected: SplitTunnelingTab) {
        tab.value = selected
    }

    /** The switch of the shown tab. */
    fun onSplitModeSwitch(on: Boolean) {
        val next = if (on) tab.value.mode else SplitTunnelMode.Off
        if (modeChangeConfirmation(splitTunnelingRepository.splitMode.value, next) != null) {
            pendingMode.value = next
        } else {
            applyMode(next)
        }
    }

    fun onConfirmModeChange() {
        val next = pendingMode.value ?: return
        pendingMode.value = null
        applyMode(next)
    }

    fun onCancelModeChange() {
        pendingMode.value = null
    }

    private fun applyMode(mode: SplitTunnelMode) {
        viewModelScope.launch(dispatcher) { splitTunnelingRepository.setSplitMode(mode) }
    }

    fun onAddAppClick(packageName: PackageName) {
        val shown = tab.value
        viewModelScope.launch(dispatcher) {
            when (shown) {
                SplitTunnelingTab.Bypass -> splitTunnelingRepository.addExcludedApp(packageName)
                SplitTunnelingTab.IncludeOnly -> splitTunnelingRepository.addIncludedApp(packageName)
            }
        }
    }

    fun onRemoveAppClick(packageName: PackageName) {
        val shown = tab.value
        viewModelScope.launch(dispatcher) {
            when (shown) {
                SplitTunnelingTab.Bypass -> splitTunnelingRepository.removeExcludedApp(packageName)
                SplitTunnelingTab.IncludeOnly ->
                    splitTunnelingRepository.removeIncludedApp(packageName)
            }
        }
    }

    fun onShowSystemAppsClick(show: Boolean) {
        viewModelScope.launch(dispatcher) {
            userPreferencesRepository.setShowSystemAppsSplitTunneling(show)
        }
    }
}
