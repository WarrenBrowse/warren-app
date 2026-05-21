package com.warrenbrowse.vpn.lib.repository

import android.content.Context
import android.content.SharedPreferences
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/**
 * Warren-side tunnel settings, persisted via [SharedPreferences].
 *
 * Kept separate from the legacy proto-backed [UserPreferencesRepository]
 * for two reasons:
 *   1. Mullvad's protobuf schema cannot grow without coordinated
 *      migration of the dead `mullvad-daemon` consumers.
 *   2. Warren's set of toggles (DAITA on/off, NAT-PMP on/off,
 *      multi-hop entry hop pubkey, M4.0 obfuscation flag, bypass CIDRs)
 *      is orthogonal to the upstream Mullvad surface and lives in its
 *      own namespace so we can drop the legacy layer without touching
 *      these.
 *
 * StateFlow values are seeded synchronously from disk on construction
 * so callers (e.g. [WarrenTunnelConfigBuilder]) can read them without
 * suspending.
 */
class WarrenLocalSettingsRepository(context: Context) {

    private val prefs: SharedPreferences =
        context.applicationContext.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)

    private val _daitaEnabled = MutableStateFlow(prefs.getBoolean(KEY_DAITA_ENABLED, false))
    val daitaEnabled: StateFlow<Boolean> = _daitaEnabled.asStateFlow()

    private val _natPmpEnabled = MutableStateFlow(prefs.getBoolean(KEY_NAT_PMP_ENABLED, false))
    val natPmpEnabled: StateFlow<Boolean> = _natPmpEnabled.asStateFlow()

    private val _multiHopEnabled = MutableStateFlow(prefs.getBoolean(KEY_MULTI_HOP_ENABLED, false))
    val multiHopEnabled: StateFlow<Boolean> = _multiHopEnabled.asStateFlow()

    private val _obfuscationM40 = MutableStateFlow(prefs.getBoolean(KEY_OBFUSCATION_M40, false))
    val obfuscationM40: StateFlow<Boolean> = _obfuscationM40.asStateFlow()

    fun setDaitaEnabled(enabled: Boolean) {
        prefs.edit().putBoolean(KEY_DAITA_ENABLED, enabled).apply()
        _daitaEnabled.value = enabled
    }

    fun setNatPmpEnabled(enabled: Boolean) {
        prefs.edit().putBoolean(KEY_NAT_PMP_ENABLED, enabled).apply()
        _natPmpEnabled.value = enabled
    }

    fun setMultiHopEnabled(enabled: Boolean) {
        prefs.edit().putBoolean(KEY_MULTI_HOP_ENABLED, enabled).apply()
        _multiHopEnabled.value = enabled
    }

    fun setObfuscationM40(enabled: Boolean) {
        prefs.edit().putBoolean(KEY_OBFUSCATION_M40, enabled).apply()
        _obfuscationM40.value = enabled
    }

    private companion object {
        const val PREFS_NAME = "warren_local_settings"
        const val KEY_DAITA_ENABLED = "daita_enabled"
        const val KEY_NAT_PMP_ENABLED = "nat_pmp_enabled"
        const val KEY_MULTI_HOP_ENABLED = "multi_hop_enabled"
        const val KEY_OBFUSCATION_M40 = "obfuscation_m40"
    }
}
