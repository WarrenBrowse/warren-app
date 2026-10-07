package com.warrenbrowse.vpn.feature.splittunneling.impl

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import java.text.Collator
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.WhileSubscribed
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.flow.flowOn
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import com.warrenbrowse.vpn.feature.splittunneling.impl.applist.AppData
import com.warrenbrowse.vpn.feature.splittunneling.impl.applist.ApplicationsProvider
import com.warrenbrowse.vpn.feature.splittunneling.impl.countries.countryPicker
import com.warrenbrowse.vpn.lib.common.Lc
import com.warrenbrowse.vpn.lib.common.constant.VIEW_MODEL_STOP_TIMEOUT
import com.warrenbrowse.vpn.lib.model.AppExit
import com.warrenbrowse.vpn.lib.model.AppRoute
import com.warrenbrowse.vpn.lib.model.AppRouteLine
import com.warrenbrowse.vpn.lib.model.AppRouteStatus
import com.warrenbrowse.vpn.lib.model.AppRoutingSettings
import com.warrenbrowse.vpn.lib.model.DefaultRoute
import com.warrenbrowse.vpn.lib.model.RoutingOp
import com.warrenbrowse.vpn.lib.model.SplitTunnelMode
import com.warrenbrowse.vpn.lib.model.appRouteLine
import com.warrenbrowse.vpn.lib.model.asAppRoute
import com.warrenbrowse.vpn.lib.model.countryDisplayName
import com.warrenbrowse.vpn.lib.repository.SplitTunnelingRepository
import com.warrenbrowse.vpn.lib.repository.UserPreferencesRepository
import com.warrenbrowse.vpn.lib.repository.WarrenAppRoutesStatusProvider
import com.warrenbrowse.vpn.lib.repository.WarrenRelayProvider
import com.warrenbrowse.vpn.lib.repository.WarrenRelaySummary

/**
 * App routing: the default route, the rules, and the pages that change them. Every change is a
 * plan of the settings model ([AppRoutingSettings.planAppRoute], [AppRoutingSettings.planDefaultRoute])
 * that the repository writes in order, so the precedence and the safe order stay in one place.
 */
@Suppress("TooManyFunctions", "LongParameterList")
class SplitTunnelingViewModel(
    private val isModal: Boolean,
    private val splitTunnelingRepository: SplitTunnelingRepository,
    private val userPreferencesRepository: UserPreferencesRepository,
    applicationsProvider: ApplicationsProvider,
    appRoutesStatusProvider: WarrenAppRoutesStatusProvider,
    private val relayProvider: WarrenRelayProvider,
    /** Whether the device can run per-app countries (Android 10 and newer). */
    private val countryPerAppSupported: Boolean,
    private val dispatcher: CoroutineDispatcher,
    private val countryName: (String) -> String = { countryDisplayName(it) },
    /** Whether the VPN carries the apps now; a locked app is blocked otherwise. */
    tunnelConnected: Flow<Boolean> = flowOf(true),
) : ViewModel() {

    private val page = MutableStateFlow<PageKey>(PageKey.Rules)
    private val addSearch = MutableStateFlow("")
    private val pickerSearch = MutableStateFlow("")
    private val pickerExpanded = MutableStateFlow<Set<String>>(emptySet())
    private val pending = MutableStateFlow<PendingChange?>(null)

    // The writes of one change must not interleave with the next one's.
    private val writing = Mutex()

    private val settings: Flow<AppRoutingSettings> =
        combine(
            combine(
                splitTunnelingRepository.splitMode,
                splitTunnelingRepository.excludedApps,
                splitTunnelingRepository.includedApps,
                splitTunnelingRepository.appExitsEnabled,
                splitTunnelingRepository.appExits,
            ) { mode, excluded, included, enabled, exits ->
                AppRoutingSettings(
                    mode,
                    excluded.mapTo(LinkedHashSet()) { it.value },
                    included.mapTo(LinkedHashSet()) { it.value },
                    enabled,
                    exits,
                )
            },
            splitTunnelingRepository.lockedApps,
        ) { settings, locked ->
            settings.copy(lockedApps = locked)
        }

    private val sources: Flow<Sources> =
        combine(
            flow { emit(applicationsProvider.apps()) }.flowOn(dispatcher),
            settings,
            splitTunnelingRepository.vpnOnlyForCount,
            appRoutesStatusProvider.appRoutes,
            tunnelConnected,
        ) { apps, settings, onlyForCount, statuses, connected ->
            Sources(apps, settings, onlyForCount, statuses, connected)
        }

    private val pageInputs: Flow<PageInputs> =
        combine(
            page,
            addSearch,
            userPreferencesRepository.showSystemAppsSplitTunneling(),
            pickerSearch,
            pickerExpanded,
        ) { page, search, showSystemApps, pickerSearch, expanded ->
            PageInputs(page, search, showSystemApps, pickerSearch, expanded)
        }

    val uiState: StateFlow<Lc<Loading, AppRoutingUiState>> =
        combine(sources, pageInputs, relayProvider.catalogue, pending) {
                sources,
                inputs,
                catalogue,
                pending ->
                Lc.Content(state(sources, inputs, catalogue, pending)) as Lc<Loading, AppRoutingUiState>
            }
            .stateIn(
                viewModelScope,
                SharingStarted.WhileSubscribed(VIEW_MODEL_STOP_TIMEOUT),
                Lc.Loading(Loading(isModal = isModal)),
            )

    private fun state(
        sources: Sources,
        inputs: PageInputs,
        catalogue: List<WarrenRelaySummary>,
        pending: PendingChange?,
    ): AppRoutingUiState {
        val settings = sources.settings
        val byPackage = sources.apps.associateBy { it.packageName.value }
        val collator = Collator.getInstance()
        val rules =
            settings
                .rules()
                .mapNotNull { rule ->
                    byPackage[rule.app]?.let { app ->
                        AppRuleItem(
                            app,
                            rule.route,
                            sources.line(rule.app, rule.route),
                            locked = rule.locked,
                        )
                    }
                }
                .sortedWith { a, b -> collator.compare(a.app.name, b.app.name) }
        val page =
            when (val key = inputs.page) {
                PageKey.Rules -> AppRoutingPage.Rules
                PageKey.AddApp -> {
                    val ruled = rules.mapTo(HashSet()) { it.app.packageName }
                    val needle = inputs.addSearch.trim()
                    AppRoutingPage.AddApp(
                        searchTerm = inputs.addSearch,
                        showSystemApps = inputs.showSystemApps,
                        apps =
                            sources.apps
                                .filter { it.packageName !in ruled }
                                .filter { inputs.showSystemApps || !it.isSystemApp }
                                .filter { needle.isEmpty() || it.name.contains(needle, true) }
                                .sortedWith { a, b -> collator.compare(a.name, b.name) },
                    )
                }
                is PageKey.Route -> {
                    val route = settings.routeOf(key.app.packageName.value)
                    AppRoutingPage.Route(
                        app = key.app,
                        route = route,
                        defaultRoute = settings.defaultRoute,
                        line = sources.line(key.app.packageName.value, route),
                        locked = key.app.packageName.value in settings.lockedApps,
                    )
                }
                is PageKey.Country ->
                    AppRoutingPage.Country(
                        countryPicker(
                            key.app,
                            settings.effectiveAppExits,
                            catalogue,
                            inputs.pickerSearch,
                            inputs.pickerExpanded,
                            countryName,
                        )
                    )
            }
        return AppRoutingUiState(
            defaultRoute = settings.defaultRoute,
            rules = rules,
            fullTunnelFallback =
                settings.splitMode == SplitTunnelMode.IncludeOnly && sources.onlyForCount == null,
            countrySupported = countryPerAppSupported,
            isModal = isModal,
            page = page,
            confirmation = pending?.let { NarrowingConfirmation(it.app) },
        )
    }

    /** The "Other apps go" choice. */
    fun onChooseDefault(next: DefaultRoute) {
        change(app = null) { it.planDefaultRoute(next) }
    }

    fun onOpenAddApp() {
        addSearch.value = ""
        page.value = PageKey.AddApp
    }

    fun onAddAppSearchChange(term: String) {
        addSearch.value = term
    }

    fun onShowSystemApps(show: Boolean) {
        viewModelScope.launch(dispatcher) {
            userPreferencesRepository.setShowSystemAppsSplitTunneling(show)
        }
    }

    /** Opens the route of [app], from its row or from the add page. Nothing is saved yet. */
    fun onOpenApp(app: AppData) {
        page.value = PageKey.Route(app)
    }

    /** Through the VPN or outside it, for the app whose route is open. */
    fun onChooseRoute(route: AppRoute) {
        val app = (page.value as? PageKey.Route)?.app ?: return
        change(app) { it.planAppRoute(app.packageName.value, route) }
    }

    /** Opens the countries of the app whose route is open, with the cities of its own shown. */
    /** "Never without the VPN": an app outside the VPN cannot be locked, so that asks nothing. */
    fun onSetLocked(locked: Boolean) {
        val app = (page.value as? PageKey.Route)?.app ?: return
        change(app) { settings ->
            val name = app.packageName.value
            if (locked && settings.routeOf(name) == AppRoute.Direct) emptyList()
            else settings.planAppLock(name, locked)
        }
    }

    fun onOpenCountries() {
        if (!countryPerAppSupported) return
        val app = (page.value as? PageKey.Route)?.app ?: return
        val current = splitTunnelingRepository.routingSettings().routeOf(app.packageName.value)
        pickerSearch.value = ""
        pickerExpanded.value = setOfNotNull((current as? AppRoute.Country)?.exit?.country)
        page.value = PageKey.Country(app)
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
     * Gives the app the country [exit] and goes back to its route. It only records the choice: the
     * main connection neither moves nor reconnects.
     */
    fun onChooseExit(exit: AppExit) {
        val app = (page.value as? PageKey.Country)?.app ?: return
        page.value = PageKey.Route(app)
        change(app) { it.planAppRoute(app.packageName.value, AppRoute.Country(exit)) }
    }

    /** The app follows the default again, and the list shows once more. */
    fun onRemoveRule() {
        val app = (page.value as? PageKey.Route)?.app ?: return
        page.value = PageKey.Rules
        change(app) { it.planAppRoute(app.packageName.value, it.defaultRoute.asAppRoute()) }
    }

    fun onDone() {
        page.value = PageKey.Rules
    }

    /** Goes one page back; false on the list, which the screen then leaves. */
    fun onBack(): Boolean {
        page.value =
            when (val current = page.value) {
                PageKey.Rules -> return false
                PageKey.AddApp,
                is PageKey.Route -> PageKey.Rules
                is PageKey.Country -> PageKey.Route(current.app)
            }
        return true
    }

    fun onConfirmNarrowing() {
        val change = pending.value ?: return
        pending.value = null
        viewModelScope.launch(dispatcher) {
            writing.withLock { splitTunnelingRepository.apply(change.ops) }
        }
    }

    fun onCancelNarrowing() {
        pending.value = null
    }

    /**
     * Plans a change against the settings as they are and writes it, unless it makes a few apps
     * the only ones in the VPN where every app uses it now: that one waits for the user's answer.
     */
    private fun change(app: AppData?, plan: (AppRoutingSettings) -> List<RoutingOp>) {
        viewModelScope.launch(dispatcher) {
            writing.withLock {
                val ops = plan(splitTunnelingRepository.routingSettings())
                when {
                    ops.isEmpty() -> Unit
                    splitTunnelingRepository.changeNarrowsTunnel(ops) ->
                        pending.value = PendingChange(app, ops)
                    else -> splitTunnelingRepository.apply(ops)
                }
            }
        }
    }

    private data class Sources(
        val apps: List<AppData>,
        val settings: AppRoutingSettings,
        val onlyForCount: Int?,
        val statuses: List<AppRouteStatus>,
        val tunnelConnected: Boolean,
    ) {
        /**
         * A locked app is blocked while the VPN is off, whatever its route; otherwise the state of
         * the route of an app with a country, which the others do not have.
         */
        fun line(app: String, route: AppRoute): AppRouteLine? =
            if (!tunnelConnected && app in settings.lockedApps) {
                AppRouteLine.Blocked
            } else if (route is AppRoute.Country) {
                appRouteLine(
                    settings.splitMode,
                    settings.excludedApps,
                    settings.appExits,
                    settings.appExitsEnabled,
                    statuses,
                    app,
                )
            } else {
                null
            }
    }

    private data class PageInputs(
        val page: PageKey,
        val addSearch: String,
        val showSystemApps: Boolean,
        val pickerSearch: String,
        val pickerExpanded: Set<String>,
    )

    private sealed interface PageKey {
        data object Rules : PageKey

        data object AddApp : PageKey

        data class Route(val app: AppData) : PageKey

        data class Country(val app: AppData) : PageKey
    }

    private data class PendingChange(val app: AppData?, val ops: List<RoutingOp>)
}
