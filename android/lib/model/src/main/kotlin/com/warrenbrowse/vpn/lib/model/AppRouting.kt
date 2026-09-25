package com.warrenbrowse.vpn.lib.model

/**
 * The split mode of docs/app-routing.md section 1: one at a time, and the app
 * list of the mode that is not in force is kept rather than cleared.
 */
enum class SplitTunnelMode {
    Off,

    /** "Bypass VPN": the excluded apps connect as if the VPN were off. */
    Exclude,

    /** "VPN only for": only the included apps use the VPN. */
    IncludeOnly,
}

/** Which apps the tunnel carries, once the settings met the installed apps. */
sealed interface AppRouting {
    data object AllApps : AppRouting

    /** Every app except [packages] (`VpnService.Builder.addDisallowedApplication`). */
    data class Bypass(val packages: Set<String>) : AppRouting

    /** Only [packages] (`VpnService.Builder.addAllowedApplication`). */
    data class OnlyFor(val packages: Set<String>) : AppRouting
}

/**
 * Resolve the saved split settings into what the tunnel carries.
 *
 * Include-only never reaches the platform with an empty allow list: a builder
 * given no allowed app (or only packages it cannot find) captures every app,
 * which would silently turn "VPN only for" into a full tunnel the user never
 * sees named. The guard makes that fallback explicit instead, and the fallback
 * is the full tunnel because it is the side that protects the included apps.
 */
fun resolveAppRouting(
    mode: SplitTunnelMode,
    excludedApps: Set<String>,
    includedApps: Set<String>,
    isInstalled: (String) -> Boolean,
): AppRouting =
    when (mode) {
        SplitTunnelMode.Off -> AppRouting.AllApps
        SplitTunnelMode.Exclude ->
            if (excludedApps.isEmpty()) AppRouting.AllApps else AppRouting.Bypass(excludedApps)
        SplitTunnelMode.IncludeOnly -> {
            val present = includedApps.filterTo(LinkedHashSet(), isInstalled)
            if (present.isEmpty()) AppRouting.AllApps else AppRouting.OnlyFor(present)
        }
    }
