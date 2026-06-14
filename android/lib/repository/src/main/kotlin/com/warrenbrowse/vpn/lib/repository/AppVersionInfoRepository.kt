package com.warrenbrowse.vpn.lib.repository

import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import com.warrenbrowse.vpn.lib.model.BuildVersion
import com.warrenbrowse.vpn.lib.model.VersionInfo

/**
 * Exposes whether the running app version is still supported, driving the
 * "unsupported version" in-app banner + store deep-link (and the forced-update
 * gate). The answer comes from the signed `android.json` update manifest,
 * fetched + Ed25519-verified in Rust via [WarrenJniBridge.checkVersionSupported]
 * (the same verifier the desktop app uses).
 *
 * Starts as `isSupported = true` (no false block before the first check) and
 * refreshes once on construction. The JNI call blocks on a network fetch, so it
 * runs on [ioDispatcher]. Fail-open: any error keeps `isSupported = true`.
 */
class AppVersionInfoRepository(
    private val buildVersion: BuildVersion,
    private val jniBridge: WarrenJniBridge,
    private val ioDispatcher: CoroutineDispatcher = Dispatchers.IO,
) {
    // App-lifetime singleton, so an unmanaged scope is acceptable: it is never
    // cancelled because the repository lives for the whole process.
    private val scope = CoroutineScope(SupervisorJob() + ioDispatcher)

    private val _versionInfo =
        MutableStateFlow(VersionInfo(currentVersion = buildVersion.name, isSupported = true))
    val versionInfo: StateFlow<VersionInfo> = _versionInfo.asStateFlow()

    init {
        scope.launch { refresh() }
    }

    /** Re-fetch the signed manifest and update [versionInfo]. Fail-open. */
    suspend fun refresh() {
        val supported =
            withContext(ioDispatcher) {
                runCatching { jniBridge.checkVersionSupported(buildVersion.name) }.getOrDefault(true)
            }
        _versionInfo.value =
            VersionInfo(currentVersion = buildVersion.name, isSupported = supported)
    }
}
