package com.warrenbrowse.vpn.lib.model

import kotlinx.serialization.SerializationException
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.contentOrNull

/**
 * The country an app leaves the Internet from ("Country per app", docs/app-routing.md section 2):
 * [country] is an ISO 3166-1 alpha-2 code in lowercase, [city] the relay list's city name, or null
 * for any city of the country.
 */
data class AppExit(val country: String, val city: String? = null) {
    companion object {
        /** A choice from user or stored input, or null when [country] is not two letters. */
        fun of(country: String, city: String?): AppExit? {
            val code = country.trim().lowercase()
            if (code.length != 2 || !code.all { it in 'a'..'z' }) return null
            return AppExit(code, city?.trim()?.takeIf { it.isNotEmpty() })
        }
    }
}

/**
 * The exits in force after the precedence rules of docs/app-routing.md section 1: none while the
 * tab's switch is off, and none for an app that bypasses the VPN while exclusion is in force. A
 * locked app never bypasses it. The Kotlin twin of `AppRoutingSettings::effective_app_exits`
 * (`mullvad-types`).
 */
fun effectiveAppExits(
    mode: SplitTunnelMode,
    excludedApps: Set<String>,
    appExits: Map<String, AppExit>,
    enabled: Boolean,
    lockedApps: Set<String> = emptySet(),
): Map<String, AppExit> =
    when {
        !enabled -> emptyMap()
        mode != SplitTunnelMode.Exclude -> appExits
        else -> appExits.filterKeys { it !in excludedApps || it in lockedApps }
    }

/** Where the session of one exit stands, as the native engine reports it. */
sealed interface AppRouteState {
    data object Connecting : AppRouteState

    data object Connected : AppRouteState

    /** [reason] is null for a reason this build does not know. */
    data class Unavailable(val reason: AppRouteUnavailableReason?) : AppRouteState
}

/** Why the session of an exit cannot run; its apps are blocked meanwhile. */
enum class AppRouteUnavailableReason(val wire: String) {
    TunnelDown("tunnel_down"),
    NoToken("no_token"),
    LimitReached("limit_reached"),
    NoRelay("no_relay"),
    WaitingForRoute("waiting_for_route"),

    /** This network routes none of the entry servers the route may use. */
    NoDialableNetwork("no_dialable_network"),
}

/** What the user sees for one exit: its state, the address its apps appear from, and its apps. */
data class AppRouteStatus(
    val exit: AppExit,
    val state: AppRouteState,
    val publicIp: String?,
    val apps: List<String>,
)

/** The status line under an app that has a country, the twin of the desktop `AppRouteLine`. */
sealed interface AppRouteLine {
    /** A locked app while the VPN does not carry it: it has no Internet meanwhile. */
    data object Blocked : AppRouteLine

    /** The country is saved while the tab's switch is off. */
    data object Paused : AppRouteLine

    /** The app bypasses the VPN, which wins over its country. */
    data object Bypassed : AppRouteLine

    /** No route reported yet: the VPN is not connected. */
    data object Waiting : AppRouteLine

    data object Connecting : AppRouteLine

    data class Connected(val publicIp: String?) : AppRouteLine

    data class Unavailable(val reason: AppRouteUnavailableReason?) : AppRouteLine
}

/**
 * The line under [app]. A country that is not in force says why before any route state, which
 * could be the last report of a route since stopped.
 */
@Suppress("LongParameterList")
fun appRouteLine(
    mode: SplitTunnelMode,
    excludedApps: Set<String>,
    appExits: Map<String, AppExit>,
    enabled: Boolean,
    statuses: List<AppRouteStatus>,
    app: String,
): AppRouteLine? {
    val status = statuses.firstOrNull { app in it.apps }
    return when {
        app !in appExits -> null
        mode == SplitTunnelMode.Exclude && app in excludedApps -> AppRouteLine.Bypassed
        !enabled -> AppRouteLine.Paused
        status == null -> AppRouteLine.Waiting
        else -> status.line()
    }
}

private fun AppRouteStatus.line(): AppRouteLine =
    when (val state = state) {
        AppRouteState.Connecting -> AppRouteLine.Connecting
        AppRouteState.Connected -> AppRouteLine.Connected(publicIp)
        is AppRouteState.Unavailable ->
            if (state.reason == AppRouteUnavailableReason.TunnelDown) {
                AppRouteLine.Waiting
            } else {
                AppRouteLine.Unavailable(state.reason)
            }
    }

/**
 * Tolerant reader of `WarrenJni.getAppRoutesStatus()`: `{"routes":[{"country":..,"city":..,
 * "state":"connecting"|"connected"|"unavailable","reason":..,"public_ip":..,"apps":[..]}]}`. A
 * route without a usable country is skipped, and an unreadable document reads as no route.
 */
object AppRouteStatusParser {
    private val json = Json { ignoreUnknownKeys = true }

    fun parse(raw: String): List<AppRouteStatus> {
        val root =
            try {
                json.parseToJsonElement(raw) as? JsonObject
            } catch (_: SerializationException) {
                null
            } catch (_: IllegalArgumentException) {
                null
            }
        val routes = root?.get("routes") as? JsonArray ?: return emptyList()
        return routes.mapNotNull { (it as? JsonObject)?.let(::route) }
    }

    private fun route(obj: JsonObject): AppRouteStatus? {
        val exit = obj.string("country")?.let { AppExit.of(it, obj.string("city")) }
        val state = state(obj)
        val apps =
            (obj["apps"] as? JsonArray)?.mapNotNull {
                (it as? JsonPrimitive)?.takeIf { p -> p.isString }?.content
            } ?: emptyList()
        return if (exit != null && state != null) {
            AppRouteStatus(exit, state, obj.string("public_ip"), apps)
        } else {
            null
        }
    }

    private fun state(obj: JsonObject): AppRouteState? =
        when (obj.string("state")) {
            "connecting" -> AppRouteState.Connecting
            "connected" -> AppRouteState.Connected
            "unavailable" ->
                AppRouteState.Unavailable(
                    AppRouteUnavailableReason.entries.firstOrNull {
                        it.wire == obj.string("reason")
                    }
                )
            else -> null
        }

    private fun JsonObject.string(key: String): String? =
        (this[key] as? JsonPrimitive)?.takeIf { it.isString }?.contentOrNull
}
