package com.warrenbrowse.vpn.feature.splittunneling.impl

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.WhileSubscribed
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import com.warrenbrowse.vpn.feature.splittunneling.impl.applist.AppData
import com.warrenbrowse.vpn.feature.splittunneling.impl.applist.SplitTunnelingUseCase
import com.warrenbrowse.vpn.feature.splittunneling.impl.countries.AppRoutingInputs
import com.warrenbrowse.vpn.feature.splittunneling.impl.countries.CountryPerAppUiState
import com.warrenbrowse.vpn.feature.splittunneling.impl.countries.CountryPickerUiState
import com.warrenbrowse.vpn.feature.splittunneling.impl.countries.countryPerAppSections
import com.warrenbrowse.vpn.feature.splittunneling.impl.countries.countryPicker
import com.warrenbrowse.vpn.lib.common.Lc
import com.warrenbrowse.vpn.lib.common.constant.VIEW_MODEL_STOP_TIMEOUT
import com.warrenbrowse.vpn.lib.model.AppExit
import com.warrenbrowse.vpn.lib.model.PackageName
import com.warrenbrowse.vpn.lib.model.SplitTunnelMode
import com.warrenbrowse.vpn.lib.model.countryDisplayName
import com.warrenbrowse.vpn.lib.repository.SplitTunnelingRepository
import com.warrenbrowse.vpn.lib.repository.UserPreferencesRepository
import com.warrenbrowse.vpn.lib.repository.WarrenAppRoutesStatusProvider
import com.warrenbrowse.vpn.lib.repository.WarrenRelayProvider

@Suppress("TooManyFunctions")
class SplitTunnelingViewModel(
    isModal: Boolean,
    initialTab: SplitTunnelingTab?,
    private val splitTunnelingRepository: SplitTunnelingRepository,
    private val userPreferencesRepository: UserPreferencesRepository,
    splitTunnelingUseCase: SplitTunnelingUseCase,
    appRoutesStatusProvider: WarrenAppRoutesStatusProvider,
    private val relayProvider: WarrenRelayProvider,
    /** Whether the device can run per-app countries (Android 10 and newer). */
    private val countryPerAppSupported: Boolean,
    private val dispatcher: CoroutineDispatcher,
    private val countryName: (String) -> String = { countryDisplayName(it) },
) : ViewModel() {

    // Without an explicit tab, the screen opens on the tab of the mode in
    // force, so a user sent here by the "VPN only for" label lands on its list.
    private val tab =
        MutableStateFlow(
            initialTab
                ?: if (splitTunnelingRepository.splitMode.value == SplitTunnelMode.IncludeOnly) {
                    SplitTunnelingTab.IncludeOnly
                } else {
                    SplitTunnelingTab.Bypass
                }
        )

    private val pendingMode = MutableStateFlow<SplitTunnelMode?>(null)

    private val countrySearch = MutableStateFlow("")
    private val picking = MutableStateFlow<AppData?>(null)
    private val pickerSearch = MutableStateFlow("")
    private val pickerExpanded = MutableStateFlow<Set<String>>(emptySet())
    private val pendingExit = MutableStateFlow<Pair<AppData, AppExit>?>(null)

    private val routing: Flow<AppRoutingInputs> =
        combine(
            splitTunnelingRepository.splitMode,
            splitTunnelingRepository.excludedApps,
            splitTunnelingRepository.appExits,
            splitTunnelingRepository.appExitsEnabled,
            appRoutesStatusProvider.appRoutes,
        ) { mode, excluded, exits, enabled, statuses ->
            AppRoutingInputs(mode, excluded.mapTo(HashSet()) { it.value }, exits, enabled, statuses)
        }

    private val picker: Flow<CountryPickerUiState?> =
        combine(
            picking,
            splitTunnelingRepository.appExits,
            relayProvider.catalogue,
            pickerSearch,
            pickerExpanded,
        ) { app, exits, relays, search, expanded ->
            app?.let { countryPicker(it, exits, relays, search, expanded, countryName) }
        }

    private val countrySearchAndPending: Flow<Pair<String, AppData?>> =
        combine(countrySearch, pendingExit) { search, pending -> search to pending?.first }

    private val baseState: Flow<SplitTunnelingUiState> =
        combine(
            splitTunnelingUseCase(tab),
            splitTunnelingRepository.splitMode,
            userPreferencesRepository.showSystemAppsSplitTunneling(),
            tab,
            pendingMode,
        ) { splitApps, mode, showSystemApps, shownTab, pending ->
            SplitTunnelingUiState(
                splitMode = mode,
                tab = shownTab,
                selectedApps = splitApps.selectedApps,
                otherApps = splitApps.otherApps,
                showSystemApps = showSystemApps,
                isModal = isModal,
                confirmation = pending?.let { modeChangeConfirmation(mode, it) },
            )
        }

    val uiState: StateFlow<Lc<Loading, SplitTunnelingUiState>> =
        combine(baseState, routing, countrySearchAndPending, picker) {
                base,
                routing,
                (search, onlyApp),
                picker ->
                val countryPerApp =
                    if (base.tab == SplitTunnelingTab.CountryPerApp) {
                        countryPerAppState(base, routing, search, picker)
                            .copy(onlyAppConfirmation = onlyApp)
                    } else {
                        null
                    }
                Lc.Content(base.copy(countryPerApp = countryPerApp))
            }
            .stateIn(
                viewModelScope,
                SharingStarted.WhileSubscribed(VIEW_MODEL_STOP_TIMEOUT),
                Lc.Loading(Loading(isModal = isModal)),
            )

    private fun countryPerAppState(
        base: SplitTunnelingUiState,
        routing: AppRoutingInputs,
        search: String,
        picker: CountryPickerUiState?,
    ): CountryPerAppUiState {
        val (withCountry, others) =
            countryPerAppSections(base.selectedApps, base.otherApps, routing, search)
        return CountryPerAppUiState(
            supported = countryPerAppSupported,
            enabled = routing.enabled,
            searchTerm = search,
            withCountry = withCountry,
            otherApps = others,
            picker = picker.takeIf { countryPerAppSupported },
        )
    }

    fun onSelectTab(selected: SplitTunnelingTab) {
        tab.value = selected
    }

    /** The switch of the shown tab. */
    fun onSplitModeSwitch(on: Boolean) {
        val tabMode = tab.value.mode ?: return
        val next = if (on) tabMode else SplitTunnelMode.Off
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
                SplitTunnelingTab.CountryPerApp -> Unit
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
                SplitTunnelingTab.CountryPerApp -> Unit
            }
        }
    }

    fun onShowSystemAppsClick(show: Boolean) {
        viewModelScope.launch(dispatcher) {
            userPreferencesRepository.setShowSystemAppsSplitTunneling(show)
        }
    }

    /**
     * The switch of the "Country per app" tab. It is not a split mode, so it never asks for a
     * confirmation, and below Android 10 it cannot be turned on.
     */
    fun onAppExitsSwitch(on: Boolean) {
        if (on && !countryPerAppSupported) return
        viewModelScope.launch(dispatcher) { splitTunnelingRepository.setAppExitsEnabled(on) }
    }

    fun onCountrySearchChange(term: String) {
        countrySearch.value = term
    }

    /** Opens the picker on [app], with the cities of its own country shown. */
    fun onPickCountry(app: AppData) {
        if (!countryPerAppSupported) return
        pickerSearch.value = ""
        pickerExpanded.value =
            setOfNotNull(splitTunnelingRepository.appExits.value[app.packageName.value]?.country)
        picking.value = app
        viewModelScope.launch(dispatcher) { relayProvider.refreshIfStale() }
    }

    fun onPickerSearchChange(term: String) {
        pickerSearch.value = term
    }

    fun onPickerToggleCountry(country: String) {
        pickerExpanded.value =
            pickerExpanded.value.let { if (country in it) it - country else it + country }
    }

    /**
     * Saves [exit] for the app the picker is open on and closes it. It only records the choice:
     * the main connection neither moves nor reconnects, and a switched off tab is turned on by
     * the repository. A choice that would make the app the only one in the VPN, where every app
     * uses it now, waits for the user's answer instead.
     */
    fun onChooseExit(exit: AppExit) {
        val app = picking.value ?: return
        picking.value = null
        viewModelScope.launch(dispatcher) {
            if (splitTunnelingRepository.countryChoiceNarrowsFullTunnel(app.packageName, exit)) {
                pendingExit.value = app to exit
            } else {
                splitTunnelingRepository.setAppExit(app.packageName, exit)
            }
        }
    }

    /** Applies the country held by the "only this app" confirmation. */
    fun onConfirmOnlyApp() {
        val (app, exit) = pendingExit.value ?: return
        pendingExit.value = null
        viewModelScope.launch(dispatcher) { splitTunnelingRepository.setAppExit(app.packageName, exit) }
    }

    fun onCancelOnlyApp() {
        pendingExit.value = null
    }

    fun onRemovePickedCountry() {
        val app = picking.value ?: return
        picking.value = null
        onClearAppCountry(app.packageName)
    }

    fun onDismissPicker() {
        picking.value = null
    }

    fun onClearAppCountry(packageName: PackageName) {
        viewModelScope.launch(dispatcher) { splitTunnelingRepository.clearAppExit(packageName) }
    }
}
