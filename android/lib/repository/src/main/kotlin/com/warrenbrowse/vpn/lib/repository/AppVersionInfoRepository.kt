package com.warrenbrowse.vpn.lib.repository

import kotlin.time.Duration
import kotlin.time.Duration.Companion.hours
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import com.warrenbrowse.vpn.lib.model.BuildVersion
import com.warrenbrowse.vpn.lib.model.VersionInfo

/**
 * Exposes whether the running app version is still supported, driving the
 * "unsupported version" in-app banner + store deep-link (and the forced-update
 * gate), plus the newest stable version available for an in-app "update
 * available" notification. Both answers come from one fetch of the signed
 * `android.json` update manifest, Ed25519-verified in Rust via [WarrenJniBridge]
 * (the same verifier the desktop app uses).
 *
 * Starts as `isSupported = true` with no known upgrade (no false block / no
 * false prompt before the first check) and refreshes once on construction, then
 * on a periodic timer so a long-running app eventually notices a newly published
 * release without a restart. The JNI call blocks on a network fetch, so it runs
 * on [ioDispatcher]. Fail-open on support (any error keeps `isSupported = true`);
 * fail-closed on the upgrade prompt (any error keeps `availableUpgrade = null`).
 */
class AppVersionInfoRepository(
    private val buildVersion: BuildVersion,
    private val jniBridge: WarrenJniBridge,
    private val ioDispatcher: CoroutineDispatcher = Dispatchers.IO,
    private val refreshInterval: Duration = REFRESH_INTERVAL,
) {
    // App-lifetime singleton, so an unmanaged scope is acceptable: it is never
    // cancelled because the repository lives for the whole process.
    private val scope = CoroutineScope(SupervisorJob() + ioDispatcher)

    private val _versionInfo =
        MutableStateFlow(VersionInfo(currentVersion = buildVersion.name, isSupported = true))
    val versionInfo: StateFlow<VersionInfo> = _versionInfo.asStateFlow()

    init {
        scope.launch {
            while (isActive) {
                refresh()
                delay(refreshInterval)
            }
        }
    }

    /**
     * Re-fetch the signed manifest once and update both answers in
     * [versionInfo]. Fail-open on the support flag, fail-closed on the upgrade
     * prompt.
     */
    suspend fun refresh() {
        val verdict =
            withContext(ioDispatcher) {
                runCatching { jniBridge.fetchVersionInfo(buildVersion.name) }
                    .getOrDefault(WarrenVersionVerdict.UNKNOWN)
            }
        _versionInfo.value =
            VersionInfo(
                currentVersion = buildVersion.name,
                isSupported = verdict.isSupported,
                availableUpgrade = verdict.latestAvailable,
            )
    }

    companion object {
        private val REFRESH_INTERVAL = 6.hours
    }
}
