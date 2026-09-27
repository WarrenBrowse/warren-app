package com.warrenbrowse.vpn.feature.splittunneling.impl.applist

import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.flowOn
import com.warrenbrowse.vpn.feature.splittunneling.impl.SplitTunnelingTab
import com.warrenbrowse.vpn.lib.model.AppExit
import com.warrenbrowse.vpn.lib.model.PackageName
import com.warrenbrowse.vpn.lib.repository.SplitTunnelingRepository
import com.warrenbrowse.vpn.lib.repository.UserPreferencesRepository

class SplitTunnelingUseCase(
    private val splitTunnelingRepository: SplitTunnelingRepository,
    private val applicationsProvider: ApplicationsProvider,
    private val preferencesRepository: UserPreferencesRepository,
    private val dispatcher: CoroutineDispatcher,
) {
    /**
     * The installed apps, split by whether they are on the list of the [tab] shown. On the
     * "Country per app" tab, that list is the apps with a saved country.
     */
    operator fun invoke(tab: Flow<SplitTunnelingTab>): Flow<SplitApps> =
        combine(
                flow { emit(applicationsProvider.apps()) },
                combine(
                    splitTunnelingRepository.excludedApps,
                    splitTunnelingRepository.includedApps,
                    splitTunnelingRepository.appExits,
                    ::Lists,
                ),
                preferencesRepository.showSystemAppsSplitTunneling(),
                tab,
            ) { allApps, lists, showSystemApps, shownTab ->
                val chosen =
                    when (shownTab) {
                        SplitTunnelingTab.Bypass -> lists.excluded
                        SplitTunnelingTab.CountryPerApp ->
                            lists.appExits.keys.mapTo(HashSet(), ::PackageName)
                        SplitTunnelingTab.IncludeOnly -> lists.included
                    }
                SplitApps(
                    allApps =
                        if (showSystemApps) allApps
                        else allApps.filter { !it.isSystemApp || it.packageName in chosen },
                    chosen = chosen,
                )
            }
            .flowOn(dispatcher)

    private data class Lists(
        val excluded: Set<PackageName>,
        val included: Set<PackageName>,
        val appExits: Map<String, AppExit>,
    )
}

data class SplitApps(private val allApps: List<AppData>, private val chosen: Set<PackageName>) {
    val selectedApps: List<AppData>
    val otherApps: List<AppData>

    init {
        allApps
            .partition { appData -> chosen.contains(appData.packageName) }
            .also { (selected, others) ->
                selectedApps = selected
                otherApps = others
            }
    }
}
