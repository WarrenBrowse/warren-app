package com.warrenbrowse.vpn.feature.splittunneling.impl.applist

import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.flowOn
import com.warrenbrowse.vpn.feature.splittunneling.impl.SplitTunnelingTab
import com.warrenbrowse.vpn.lib.model.PackageName
import com.warrenbrowse.vpn.lib.repository.SplitTunnelingRepository
import com.warrenbrowse.vpn.lib.repository.UserPreferencesRepository

class SplitTunnelingUseCase(
    private val splitTunnelingRepository: SplitTunnelingRepository,
    private val applicationsProvider: ApplicationsProvider,
    private val preferencesRepository: UserPreferencesRepository,
    private val dispatcher: CoroutineDispatcher,
) {
    /** The installed apps, split by whether they are on the list of the [tab] shown. */
    operator fun invoke(tab: Flow<SplitTunnelingTab>): Flow<SplitApps> =
        combine(
                flow { emit(applicationsProvider.apps()) },
                splitTunnelingRepository.excludedApps,
                splitTunnelingRepository.includedApps,
                preferencesRepository.showSystemAppsSplitTunneling(),
                tab,
            ) { allApps, excluded, included, showSystemApps, shownTab ->
                val chosen =
                    when (shownTab) {
                        SplitTunnelingTab.Bypass -> excluded
                        SplitTunnelingTab.IncludeOnly -> included
                    }
                SplitApps(
                    allApps =
                        if (showSystemApps) allApps
                        else allApps.filter { !it.isSystemApp || it.packageName in chosen },
                    chosen = chosen,
                )
            }
            .flowOn(dispatcher)
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
