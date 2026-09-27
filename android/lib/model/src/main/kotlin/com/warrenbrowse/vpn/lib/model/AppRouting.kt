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
 * (docs/app-routing.md section 1, rule 2).
 */
fun resolveAppRouting(
    mode: SplitTunnelMode,
    excludedApps: Set<String>,
    includedApps: Set<String>,
    appsWithCountry: Set<String> = emptySet(),
    isInstalled: (String) -> Boolean,
): AppRouting =
    when (mode) {
        SplitTunnelMode.Off -> AppRouting.AllApps
        SplitTunnelMode.Exclude ->
            if (excludedApps.isEmpty()) AppRouting.AllApps else AppRouting.Bypass(excludedApps)
        SplitTunnelMode.IncludeOnly -> {
            val present = (includedApps + appsWithCountry).filterTo(LinkedHashSet(), isInstalled)
            if (present.isEmpty()) AppRouting.AllApps else AppRouting.OnlyFor(present)
        }
    }

/**
 * Whether choosing [exit] for [app] turns the full-tunnel fallback of include-only into a list, so
 * that every other app leaves the VPN: the list holds no app on the device, and the chosen app
 * would be the first. Choosing a country turns the tab's switch on, so every saved country counts
 * after the choice. Computed with [effectiveAppExits] and [resolveAppRouting], the functions the
 * tunnel itself resolves its apps with, so the question cannot drift from the answer.
 */
@Suppress("LongParameterList")
fun countryChoiceNarrowsFullTunnel(
    mode: SplitTunnelMode,
    excludedApps: Set<String>,
    includedApps: Set<String>,
    appExits: Map<String, AppExit>,
    appExitsEnabled: Boolean,
    app: String,
    exit: AppExit,
    isInstalled: (String) -> Boolean,
): Boolean {
    fun routing(exits: Map<String, AppExit>, enabled: Boolean) =
        resolveAppRouting(
            mode,
            excludedApps,
            includedApps,
            effectiveAppExits(mode, excludedApps, exits, enabled).keys,
            isInstalled,
        )
    return routing(appExits, appExitsEnabled) == AppRouting.AllApps &&
        routing(appExits + (app to exit), enabled = true) is AppRouting.OnlyFor
}

/**
 * Whether turning the "Country per app" switch on turns the full-tunnel fallback of include-only
 * into a list, so that every other app leaves the VPN: the list holds no app on the device, and the
 * saved countries of apps on the device come into force. The same question as
 * [countryChoiceNarrowsFullTunnel], asked with no new country.
 */
fun appExitsSwitchNarrowsFullTunnel(
    mode: SplitTunnelMode,
    excludedApps: Set<String>,
    includedApps: Set<String>,
    appExits: Map<String, AppExit>,
    appExitsEnabled: Boolean,
    isInstalled: (String) -> Boolean,
): Boolean {
    fun routing(enabled: Boolean) =
        resolveAppRouting(
            mode,
            excludedApps,
            includedApps,
            effectiveAppExits(mode, excludedApps, appExits, enabled).keys,
            isInstalled,
        )
    return routing(appExitsEnabled) == AppRouting.AllApps &&
        routing(enabled = true) is AppRouting.OnlyFor
}
