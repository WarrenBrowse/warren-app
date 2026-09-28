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
import com.warrenbrowse.vpn.lib.model.AppRoutingSettings
import com.warrenbrowse.vpn.lib.model.PackageName
import com.warrenbrowse.vpn.lib.model.RoutingOp
import com.warrenbrowse.vpn.lib.model.SplitTunnelMode
import com.warrenbrowse.vpn.lib.model.changeNarrowsTunnel
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

    /** Whether the saved countries are in force. */
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

    /** The saved settings as they are now, for App routing to plan a change against. */
    fun routingSettings(): AppRoutingSettings =
        AppRoutingSettings(
            splitMode = settings.splitMode.value,
            excludedApps = settings.excludedApps.value,
            includedApps = settings.includedApps.value,
            appExitsEnabled = settings.appExitsEnabled.value,
            appExits = settings.appExits.value,
        )

    /**
     * Writes [ops] one at a time and in their order: the tunnel follows each write on its own, and
     * the order is what keeps every state in between safe ([AppRoutingSettings.planAppRoute]).
     */
    fun apply(ops: List<RoutingOp>) {
        for (op in ops) {
            when (op) {
                is RoutingOp.SetSplitMode -> settings.setSplitMode(op.mode)
                is RoutingOp.AddExcluded -> settings.addExcludedApp(op.app)
                is RoutingOp.RemoveExcluded -> settings.removeExcludedApp(op.app)
                is RoutingOp.AddIncluded -> settings.addIncludedApp(op.app)
                is RoutingOp.RemoveIncluded -> settings.removeIncludedApp(op.app)
                is RoutingOp.SetExit -> settings.setAppExit(op.app, op.exit)
                is RoutingOp.ClearExit -> settings.clearAppExit(op.app)
                is RoutingOp.SetExitsEnabled -> settings.setAppExitsEnabled(op.enabled)
            }
        }
    }

    /**
     * Whether [ops] would turn a tunnel that carries every app into a list, where include-only runs
     * as a full tunnel because none of its apps is on the device, so the screen asks first. It asks
     * the package manager, so only ever off the main thread.
     */
    fun changeNarrowsTunnel(ops: List<RoutingOp>): Boolean {
        val now = routingSettings()
        return changeNarrowsTunnel(now, now.apply(ops), isAppInstalled)
    }

    private fun StateFlow<Set<String>>.asPackageNames(): StateFlow<Set<PackageName>> =
        map { set -> set.mapTo(LinkedHashSet(), ::PackageName) }
            .stateIn(scope, SharingStarted.Eagerly, value.mapTo(LinkedHashSet(), ::PackageName))
}
