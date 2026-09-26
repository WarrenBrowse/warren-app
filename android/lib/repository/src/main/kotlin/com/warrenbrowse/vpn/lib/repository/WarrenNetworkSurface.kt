package com.warrenbrowse.vpn.lib.repository

import com.warrenbrowse.vpn.lib.model.WarrenNetworkStats
import kotlinx.coroutines.flow.StateFlow

/**
 * Public environment descriptor served by the API (`GET /v1/network`).
 * Display data only: the compiled flavor stays the authority on WHICH
 * build this is; this feed supplies the live figures (e.g. the beta
 * bandwidth cap) the beta UI shows.
 */
data class WarrenNetworkInfo(
    val environment: String,
    val degraded: Boolean,
    /** Default per-subscriber bandwidth cap in bits per second, null = no cap. */
    val defaultRateBps: Long?,
    val paymentsEnabled: Boolean,
)

/**
 * Process-wide network-info feed. `null` until the first successful
 * fetch (or when the API predates the endpoint). Implemented in the
 * app module (JNI-backed fetch with retry), consumed by lib UI.
 */
interface WarrenNetworkInfoProvider {
    val networkInfo: StateFlow<WarrenNetworkInfo?>
}

/**
 * The public network stats feed (`GET /v1/network/stats`). [snapshot] is the last good one, kept
 * through failures so a surface greys it rather than showing zeros; [availability] is what the
 * latest fetch said.
 */
data class WarrenNetworkStatsState(
    val snapshot: WarrenNetworkStats?,
    val availability: NetworkStatsAvailability,
) {
    companion object {
        val INITIAL = WarrenNetworkStatsState(null, NetworkStatsAvailability.LOADING)
    }
}

enum class NetworkStatsAvailability {
    /** Nothing fetched yet. */
    LOADING,
    AVAILABLE,

    /** The API does not serve a snapshot this build reads (not deployed yet, or a newer schema). */
    UNAVAILABLE,

    /** The latest fetch failed transiently. */
    FAILING,
}

/**
 * Collecting [state] is what makes the feed poll: it fetches once per window while at least one
 * collector is active and never otherwise, so a surface collects it only while it shows the
 * figures and the app is in the foreground (`collectAsStateWithLifecycle`). A periodic request is
 * itself a fingerprint of a running client, so nothing polls in the background.
 */
interface WarrenNetworkStatsProvider {
    val state: StateFlow<WarrenNetworkStatsState>
}

/**
 * Compile-time product facts of this build, surfaced to lib modules
 * that cannot read the app's BuildConfig. Bound in the app DI from the
 * Gradle flavor.
 */
data class WarrenProductFlags(val isBeta: Boolean)
