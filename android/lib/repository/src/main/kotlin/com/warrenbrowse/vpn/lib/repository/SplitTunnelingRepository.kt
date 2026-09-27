package com.warrenbrowse.vpn.lib.repository

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import com.warrenbrowse.vpn.lib.model.AppExit
import com.warrenbrowse.vpn.lib.model.AppRouting
import com.warrenbrowse.vpn.lib.model.PackageName
import com.warrenbrowse.vpn.lib.model.SplitTunnelMode
import com.warrenbrowse.vpn.lib.model.effectiveAppExits
import com.warrenbrowse.vpn.lib.model.resolveAppRouting

/**
 * Warren-native split tunnelling: the split mode and both app lists are
 * persisted by [WarrenLocalSettingsRepository] and applied by the tunnel
 * service through `VpnService.Builder` (see the service's TUN plan). An
 * excluded app goes OUTSIDE the tunnel in [SplitTunnelMode.Exclude]; an
 * included app is one of the only apps INSIDE it in
 * [SplitTunnelMode.IncludeOnly]. An app with a country leaves the Internet
 * from that country through a route session of its own ("Country per app",
 * docs/app-routing.md section 2), and in include-only that choice alone puts
 * it in the VPN.
 */
class SplitTunnelingRepository(
    private val settings: WarrenLocalSettingsRepository,
    // A PackageManager lookup, so it is only ever asked off the main thread.
    private val isAppInstalled: (String) -> Boolean,
) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)

    val splitMode: StateFlow<SplitTunnelMode> = settings.splitMode

    val excludedApps: StateFlow<Set<PackageName>> = settings.excludedApps.asPackageNames()

    val includedApps: StateFlow<Set<PackageName>> = settings.includedApps.asPackageNames()

    /** The switch of the "Country per app" tab. */
    val appExitsEnabled: StateFlow<Boolean> = settings.appExitsEnabled

    /** Every saved country, in force or not, by package name. */
    val appExits: StateFlow<Map<String, AppExit>> = settings.appExits

    /** The countries in force after the precedence rules, by package name. */
    val effectiveAppExits: StateFlow<Map<String, AppExit>> =
        combine(
                settings.splitMode,
                settings.excludedApps,
                settings.appExits,
                settings.appExitsEnabled,
            ) { mode, excluded, exits, enabled ->
                effectiveAppExits(mode, excluded, exits, enabled)
            }
            .stateIn(
                scope,
                SharingStarted.Eagerly,
                effectiveAppExits(
                    settings.splitMode.value,
                    settings.excludedApps.value,
                    settings.appExits.value,
                    settings.appExitsEnabled.value,
                ),
            )

    /**
     * How many apps "VPN only for" carries, or null while the tunnel carries
     * every app (another mode, or no included app on the device).
     */
    val vpnOnlyForCount: StateFlow<Int?> =
        combine(
                settings.splitMode,
                settings.excludedApps,
                settings.includedApps,
                effectiveAppExits,
            ) { mode, excluded, included, exits ->
                when (
                    val routing =
                        resolveAppRouting(mode, excluded, included, exits.keys, isAppInstalled)
                ) {
                    is AppRouting.OnlyFor -> routing.packages.size
                    AppRouting.AllApps,
                    is AppRouting.Bypass -> null
                }
            }
            .stateIn(scope, SharingStarted.Eagerly, null)

    fun setSplitMode(mode: SplitTunnelMode) = settings.setSplitMode(mode)

    fun addExcludedApp(app: PackageName) = settings.addExcludedApp(app.value)

    fun removeExcludedApp(app: PackageName) = settings.removeExcludedApp(app.value)

    fun addIncludedApp(app: PackageName) = settings.addIncludedApp(app.value)

    fun removeIncludedApp(app: PackageName) = settings.removeIncludedApp(app.value)

    fun setAppExitsEnabled(enabled: Boolean) = settings.setAppExitsEnabled(enabled)

    /**
     * Chooses [exit] for [app]. Choosing a country is asking for it, so a
     * switched off tab is turned on, as on desktop.
     */
    fun setAppExit(app: PackageName, exit: AppExit) {
        settings.setAppExit(app.value, exit)
        if (!settings.appExitsEnabled.value) settings.setAppExitsEnabled(true)
    }

    fun clearAppExit(app: PackageName) = settings.clearAppExit(app.value)

    private fun StateFlow<Set<String>>.asPackageNames(): StateFlow<Set<PackageName>> =
        map { set -> set.mapTo(LinkedHashSet(), ::PackageName) }
            .stateIn(scope, SharingStarted.Eagerly, value.mapTo(LinkedHashSet(), ::PackageName))
}
