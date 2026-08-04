package com.warrenbrowse.vpn.lib.repository

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
 * Compile-time product facts of this build, surfaced to lib modules
 * that cannot read the app's BuildConfig. Bound in the app DI from the
 * Gradle flavor.
 */
data class WarrenProductFlags(val isBeta: Boolean)
