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
 *
 * [appsWithCountry] are the apps whose country is in force ([effectiveAppExits]): in include-only
 * they are tunneled as well, since choosing a country for an app is enough to put it in the VPN
 * (docs/app-routing.md section 1, rule 2). [lockedApps] are never outside the tunnel: never
 * bypassing, and carried in include-only (section 8).
 */
fun resolveAppRouting(
    mode: SplitTunnelMode,
    excludedApps: Set<String>,
    includedApps: Set<String>,
    appsWithCountry: Set<String> = emptySet(),
    lockedApps: Set<String> = emptySet(),
    isInstalled: (String) -> Boolean,
): AppRouting =
    when (mode) {
        SplitTunnelMode.Off -> AppRouting.AllApps
        SplitTunnelMode.Exclude -> {
            val bypassing = excludedApps - lockedApps
            if (bypassing.isEmpty()) AppRouting.AllApps else AppRouting.Bypass(bypassing)
        }
        SplitTunnelMode.IncludeOnly -> {
            val present =
                (includedApps + appsWithCountry + lockedApps).filterTo(LinkedHashSet(), isInstalled)
            if (present.isEmpty()) AppRouting.AllApps else AppRouting.OnlyFor(present)
        }
    }

/**
 * The interface that holds the locked apps while the tunnel does not carry them: exactly the
 * locked apps on the device, or null when there is none, since a builder given no allowed app
 * captures every app.
 */
fun lockGuardRouting(lockedApps: Set<String>, isInstalled: (String) -> Boolean): AppRouting? {
    val present = lockedApps.filterTo(LinkedHashSet(), isInstalled)
    return if (present.isEmpty()) null else AppRouting.OnlyFor(present)
}
