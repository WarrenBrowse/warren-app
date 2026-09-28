package com.warrenbrowse.vpn.lib.model

/** What the apps without a rule do: the "Other apps go" choice of App routing. */
enum class DefaultRoute {
    Vpn,
    Direct,
}

/** The one route an app takes. */
sealed interface AppRoute {
    data object Vpn : AppRoute

    data object Direct : AppRoute

    data class Country(val exit: AppExit) : AppRoute
}

/** An app whose route differs from the default. */
data class AppRule(val app: String, val route: AppRoute)

/** One settings write; a change is a list of them, applied in order. */
sealed interface RoutingOp {
    data class SetSplitMode(val mode: SplitTunnelMode) : RoutingOp

    data class AddExcluded(val app: String) : RoutingOp

    data class RemoveExcluded(val app: String) : RoutingOp

    data class AddIncluded(val app: String) : RoutingOp

    data class RemoveIncluded(val app: String) : RoutingOp

    data class SetExit(val app: String, val exit: AppExit) : RoutingOp

    data class ClearExit(val app: String) : RoutingOp

    data class SetExitsEnabled(val enabled: Boolean) : RoutingOp
}

/**
 * The saved routing settings of docs/app-routing.md section 1, by package name. App routing shows
 * one list where each app has a single route and the apps without one follow the default; these
 * functions translate between that view and the settings both ways, so the precedence stays the
 * tunnel's. The Kotlin twin of the desktop `src/shared/app-routing.ts`.
 */
data class AppRoutingSettings(
    val splitMode: SplitTunnelMode = SplitTunnelMode.Off,
    val excludedApps: Set<String> = emptySet(),
    val includedApps: Set<String> = emptySet(),
    val appExitsEnabled: Boolean = false,
    val appExits: Map<String, AppExit> = emptyMap(),
) {
    val defaultRoute: DefaultRoute
        get() = if (splitMode == SplitTunnelMode.IncludeOnly) DefaultRoute.Direct else DefaultRoute.Vpn

    /** The countries in force after the precedence rules. */
    val effectiveAppExits: Map<String, AppExit>
        get() = effectiveAppExits(splitMode, excludedApps, appExits, appExitsEnabled)

    /** The route the tunnel gives [app], after the precedence rules. */
    fun routeOf(app: String): AppRoute {
        val exit = effectiveAppExits[app]
        return when {
            splitMode == SplitTunnelMode.Exclude && app in excludedApps -> AppRoute.Direct
            exit != null -> AppRoute.Country(exit)
            splitMode != SplitTunnelMode.IncludeOnly -> AppRoute.Vpn
            app in includedApps -> AppRoute.Vpn
            else -> AppRoute.Direct
        }
    }

    /** The apps whose route differs from the default, in list order. */
    fun rules(): List<AppRule> {
        val candidates = LinkedHashSet<String>()
        when (splitMode) {
            SplitTunnelMode.Exclude -> candidates += excludedApps
            SplitTunnelMode.IncludeOnly -> candidates += includedApps
            SplitTunnelMode.Off -> Unit
        }
        if (appExitsEnabled) candidates += appExits.keys
        val fallback = defaultRoute.asAppRoute()
        return candidates.map { AppRule(it, routeOf(it)) }.filter { it.route != fallback }
    }

    /**
     * The writes that give [app] the route [next], in an order where every state in between routes
     * the app the old way or the new way, never a third, and no other app moves: the tunnel
     * follows each write on its own, so each state carries traffic.
     */
    @Suppress("CyclomaticComplexMethod", "NestedBlockDepth")
    fun planAppRoute(app: String, next: AppRoute): List<RoutingOp> {
        if (routeOf(app) == next) return emptyList()
        val fallback = defaultRoute
        val excluded = app in excludedApps
        val included = app in includedApps
        val hasExit = app in appExits
        val excludedLeft = excludedApps - app
        val ops = mutableListOf<RoutingOp>()

        // Leaving the list turns bypass off with its last app, as it was turned on with its first.
        fun leaveBypass() {
            if (!excluded) return
            ops += RoutingOp.RemoveExcluded(app)
            if (splitMode == SplitTunnelMode.Exclude && excludedLeft.isEmpty()) {
                ops += RoutingOp.SetSplitMode(SplitTunnelMode.Off)
            }
        }

        when (next) {
            AppRoute.Direct ->
                if (fallback == DefaultRoute.Direct) {
                    // Direct is "no rule" here.
                    if (included) ops += RoutingOp.RemoveIncluded(app)
                    if (hasExit) ops += RoutingOp.ClearExit(app)
                } else {
                    // Bypass switched on again must not bring back apps saved while it was off:
                    // nobody sees them in the list.
                    if (splitMode != SplitTunnelMode.Exclude) {
                        excludedLeft.forEach { ops += RoutingOp.RemoveExcluded(it) }
                    }
                    if (!excluded) ops += RoutingOp.AddExcluded(app)
                    if (splitMode != SplitTunnelMode.Exclude) {
                        ops += RoutingOp.SetSplitMode(SplitTunnelMode.Exclude)
                    }
                    // Bypass wins over a country, so the country goes once the app bypasses.
                    if (hasExit) ops += RoutingOp.ClearExit(app)
                }
            is AppRoute.Country -> {
                // The same for countries saved while they were off.
                if (!appExitsEnabled) {
                    appExits.keys.filter { it != app }.forEach { ops += RoutingOp.ClearExit(it) }
                }
                ops += RoutingOp.SetExit(app, next.exit)
                if (!appExitsEnabled) ops += RoutingOp.SetExitsEnabled(true)
                // The country is in force from here, so the app leaves its list.
                if (included) ops += RoutingOp.RemoveIncluded(app)
                leaveBypass()
            }
            AppRoute.Vpn ->
                if (fallback == DefaultRoute.Direct) {
                    // Included first: an app with a country stays in the VPN throughout.
                    if (!included) ops += RoutingOp.AddIncluded(app)
                    if (hasExit) ops += RoutingOp.ClearExit(app)
                } else {
                    // Bypass wins over a country, so the country goes while the app still bypasses.
                    if (hasExit) ops += RoutingOp.ClearExit(app)
                    leaveBypass()
                }
        }
        return ops
    }

    /**
     * The writes that switch what the apps without a rule do. The countries stay; both lists are
     * emptied, since a list kept from an earlier choice would come back as rules nobody just made.
     * Toward the VPN the mode goes first and toward direct it goes last, so every app is in the
     * VPN while the lists are emptied.
     */
    fun planDefaultRoute(next: DefaultRoute): List<RoutingOp> {
        if (defaultRoute == next) return emptyList()
        val clearLists =
            excludedApps.map(RoutingOp::RemoveExcluded) + includedApps.map(RoutingOp::RemoveIncluded)
        return when (next) {
            DefaultRoute.Vpn -> listOf(RoutingOp.SetSplitMode(SplitTunnelMode.Off)) + clearLists
            DefaultRoute.Direct -> clearLists + RoutingOp.SetSplitMode(SplitTunnelMode.IncludeOnly)
        }
    }

    /** What the settings hold once [ops] are applied, to reason about a change before making it. */
    fun apply(ops: List<RoutingOp>): AppRoutingSettings =
        ops.fold(this) { state, op ->
            when (op) {
                is RoutingOp.SetSplitMode -> state.copy(splitMode = op.mode)
                is RoutingOp.AddExcluded -> state.copy(excludedApps = state.excludedApps + op.app)
                is RoutingOp.RemoveExcluded -> state.copy(excludedApps = state.excludedApps - op.app)
                is RoutingOp.AddIncluded -> state.copy(includedApps = state.includedApps + op.app)
                is RoutingOp.RemoveIncluded -> state.copy(includedApps = state.includedApps - op.app)
                is RoutingOp.SetExit -> state.copy(appExits = state.appExits + (op.app to op.exit))
                is RoutingOp.ClearExit -> state.copy(appExits = state.appExits - op.app)
                is RoutingOp.SetExitsEnabled -> state.copy(appExitsEnabled = op.enabled)
            }
        }

    /** What the tunnel carries under these settings, with the include-only guard of section 3.4. */
    fun tunnelRouting(isInstalled: (String) -> Boolean): AppRouting =
        resolveAppRouting(splitMode, excludedApps, includedApps, effectiveAppExits.keys, isInstalled)
}

fun DefaultRoute.asAppRoute(): AppRoute =
    when (this) {
        DefaultRoute.Vpn -> AppRoute.Vpn
        DefaultRoute.Direct -> AppRoute.Direct
    }

/**
 * Whether going from [before] to [after] turns a tunnel that carries every app, or every app but
 * the bypassing ones, into a list, so that the rest of the device leaves the VPN at once. On
 * Android an include-only list with no app on the device runs as a full tunnel (docs/app-routing.md
 * section 3.4), so the first rule with direct as the default is such a change, and the screen asks
 * first. Computed with [resolveAppRouting], the function the tunnel resolves its apps with, so the
 * question cannot drift from the answer. It asks the package manager, so only off the main thread.
 */
fun changeNarrowsTunnel(
    before: AppRoutingSettings,
    after: AppRoutingSettings,
    isInstalled: (String) -> Boolean,
): Boolean =
    before.tunnelRouting(isInstalled) !is AppRouting.OnlyFor &&
        after.tunnelRouting(isInstalled) is AppRouting.OnlyFor
