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

    /** "udp" or "tcp". */
    private val _natPmpProtocol = MutableStateFlow(prefs.getString(KEY_NAT_PMP_PROTOCOL, NAT_PMP_PROTOCOL_UDP) ?: NAT_PMP_PROTOCOL_UDP)
    val natPmpProtocol: StateFlow<String> = _natPmpProtocol.asStateFlow()

    /** Requested external port; 0 = let the gateway pick. */
    private val _natPmpExternalPort = MutableStateFlow(prefs.getInt(KEY_NAT_PMP_EXTERNAL_PORT, 0))
    val natPmpExternalPort: StateFlow<Int> = _natPmpExternalPort.asStateFlow()

    /** Requested mapping lifetime in seconds. */
    private val _natPmpLifetimeSecs = MutableStateFlow(prefs.getInt(KEY_NAT_PMP_LIFETIME_SECS, NAT_PMP_DEFAULT_LIFETIME_SECS))
    val natPmpLifetimeSecs: StateFlow<Int> = _natPmpLifetimeSecs.asStateFlow()

    private val _multiHopEnabled = MutableStateFlow(prefs.getBoolean(KEY_MULTI_HOP_ENABLED, false))
    val multiHopEnabled: StateFlow<Boolean> = _multiHopEnabled.asStateFlow()

    /** Preferred entry-relay country (ISO alpha-2), null/empty = automatic. */
    private val _entryCountry = MutableStateFlow(prefs.getString(KEY_ENTRY_COUNTRY, null))
    val entryCountry: StateFlow<String?> = _entryCountry.asStateFlow()

    /** Preferred exit-relay country (ISO alpha-2), null/empty = automatic. */
    private val _exitCountry = MutableStateFlow(prefs.getString(KEY_EXIT_COUNTRY, null))
    val exitCountry: StateFlow<String?> = _exitCountry.asStateFlow()

    private val _obfuscationM40 = MutableStateFlow(prefs.getBoolean(KEY_OBFUSCATION_M40, false))
    val obfuscationM40: StateFlow<Boolean> = _obfuscationM40.asStateFlow()

    // --- Privacy-leak controls (P0) ---

    /** Route IPv6 through the tunnel. `false` (default) blackholes IPv6. */
    private val _ipv6Enabled = MutableStateFlow(prefs.getBoolean(KEY_IPV6_ENABLED, false))
    val ipv6Enabled: StateFlow<Boolean> = _ipv6Enabled.asStateFlow()

    /** Kill switch: keep traffic blocked when the tunnel drops. */
    private val _lockdownMode = MutableStateFlow(prefs.getBoolean(KEY_LOCKDOWN_MODE, false))
    val lockdownMode: StateFlow<Boolean> = _lockdownMode.asStateFlow()

    /** Local network sharing: let LAN hosts bypass the tunnel. */
    private val _allowLan = MutableStateFlow(prefs.getBoolean(KEY_ALLOW_LAN, false))
    val allowLan: StateFlow<Boolean> = _allowLan.asStateFlow()

    /** DNS mode: [DNS_STATE_DEFAULT] or [DNS_STATE_CUSTOM]. */
    private val _dnsState = MutableStateFlow(prefs.getString(KEY_DNS_STATE, DNS_STATE_DEFAULT) ?: DNS_STATE_DEFAULT)
    val dnsState: StateFlow<String> = _dnsState.asStateFlow()

    /** Custom DNS resolver addresses (only used in [DNS_STATE_CUSTOM]). */
    private val _customDnsServers = MutableStateFlow(readCustomDnsServers())
    val customDnsServers: StateFlow<List<String>> = _customDnsServers.asStateFlow()

    private val _blockAds = MutableStateFlow(prefs.getBoolean(KEY_DNS_BLOCK_ADS, false))
    val blockAds: StateFlow<Boolean> = _blockAds.asStateFlow()

    private val _blockTrackers = MutableStateFlow(prefs.getBoolean(KEY_DNS_BLOCK_TRACKERS, false))
    val blockTrackers: StateFlow<Boolean> = _blockTrackers.asStateFlow()

    private val _blockMalware = MutableStateFlow(prefs.getBoolean(KEY_DNS_BLOCK_MALWARE, false))
    val blockMalware: StateFlow<Boolean> = _blockMalware.asStateFlow()

    private val _blockAdultContent = MutableStateFlow(prefs.getBoolean(KEY_DNS_BLOCK_ADULT, false))
    val blockAdultContent: StateFlow<Boolean> = _blockAdultContent.asStateFlow()

    private val _blockGambling = MutableStateFlow(prefs.getBoolean(KEY_DNS_BLOCK_GAMBLING, false))
    val blockGambling: StateFlow<Boolean> = _blockGambling.asStateFlow()

    private val _blockSocialMedia = MutableStateFlow(prefs.getBoolean(KEY_DNS_BLOCK_SOCIAL, false))
    val blockSocialMedia: StateFlow<Boolean> = _blockSocialMedia.asStateFlow()

    /**
     * User-selected exit relay identifier (16-byte stable exit_id hex).
     * `null` = picker has not been used yet; the builder falls back to
     * the first active entry in [com.warrenbrowse.vpn.app.connect.RelayCatalog].
     * Wired by the D.6 location picker UI.
     */
    private val _selectedExitId = MutableStateFlow(prefs.getString(KEY_SELECTED_EXIT_ID, null))
    val selectedExitId: StateFlow<String?> = _selectedExitId.asStateFlow()

    /**
     * Recently selected exit identifiers, most-recent-first, capped at
     * [MAX_RECENT_EXITS]. Surfaced at the top of the location picker so
     * frequently-used exits are one tap away (desktop "recents" parity).
     */
    private val _recentExitIds = MutableStateFlow(readRecentExitIds())
    val recentExitIds: StateFlow<List<String>> = _recentExitIds.asStateFlow()

    fun setDaitaEnabled(enabled: Boolean) {
        prefs.edit().putBoolean(KEY_DAITA_ENABLED, enabled).apply()
        _daitaEnabled.value = enabled
    }

    fun setNatPmpEnabled(enabled: Boolean) {
        prefs.edit().putBoolean(KEY_NAT_PMP_ENABLED, enabled).apply()
        _natPmpEnabled.value = enabled
    }

    fun setNatPmpProtocol(protocol: String) {
        val normalized = if (protocol == NAT_PMP_PROTOCOL_TCP) NAT_PMP_PROTOCOL_TCP else NAT_PMP_PROTOCOL_UDP
        prefs.edit().putString(KEY_NAT_PMP_PROTOCOL, normalized).apply()
        _natPmpProtocol.value = normalized
    }

    /** Clamp to the dynamic/private port range, or 0 for "auto". */
    fun setNatPmpExternalPort(port: Int) {
        val clamped = when {
            port <= 0 -> 0
            port in 49152..65535 -> port
            else -> 0
        }
        prefs.edit().putInt(KEY_NAT_PMP_EXTERNAL_PORT, clamped).apply()
        _natPmpExternalPort.value = clamped
    }

    fun setNatPmpLifetimeSecs(seconds: Int) {
        val clamped = seconds.coerceIn(NAT_PMP_MIN_LIFETIME_SECS, NAT_PMP_MAX_LIFETIME_SECS)
        prefs.edit().putInt(KEY_NAT_PMP_LIFETIME_SECS, clamped).apply()
        _natPmpLifetimeSecs.value = clamped
    }

    fun setMultiHopEnabled(enabled: Boolean) {
        prefs.edit().putBoolean(KEY_MULTI_HOP_ENABLED, enabled).apply()
        _multiHopEnabled.value = enabled
    }

    fun setEntryCountry(country: String?) = setCountry(KEY_ENTRY_COUNTRY, country, _entryCountry)

    fun setExitCountry(country: String?) = setCountry(KEY_EXIT_COUNTRY, country, _exitCountry)

    /** Normalize an ISO alpha-2 code (uppercase, 2 letters) or clear it. */
    private fun setCountry(key: String, country: String?, flow: MutableStateFlow<String?>) {
        val normalized = country?.trim()?.uppercase()?.takeIf { it.length == 2 && it.all(Char::isLetter) }
        val editor = prefs.edit()
        if (normalized == null) editor.remove(key) else editor.putString(key, normalized)
        editor.apply()
        flow.value = normalized
    }

    fun setObfuscationM40(enabled: Boolean) {
        prefs.edit().putBoolean(KEY_OBFUSCATION_M40, enabled).apply()
        _obfuscationM40.value = enabled
    }

    /**
     * Persist the user's exit selection. Pass `null` to clear it (the
     * builder then falls back to the first active relay in the
     * catalogue).
     */
    fun setSelectedExitId(exitId: String?) {
        val editor = prefs.edit()
        if (exitId == null) {
            editor.remove(KEY_SELECTED_EXIT_ID)
        } else {
            editor.putString(KEY_SELECTED_EXIT_ID, exitId)
        }
        editor.apply()
        _selectedExitId.value = exitId
        // Selecting (not clearing) an exit records it as recently used.
        if (exitId != null) recordRecentExit(exitId)
    }

    /** Push [exitId] to the front of the recents list (deduped, capped). */
    fun recordRecentExit(exitId: String) {
        val updated = (listOf(exitId) + _recentExitIds.value.filter { it != exitId })
            .take(MAX_RECENT_EXITS)
        prefs.edit().putString(KEY_RECENT_EXIT_IDS, updated.joinToString(RECENT_DELIMITER)).apply()
        _recentExitIds.value = updated
    }

    private fun readRecentExitIds(): List<String> =
        prefs.getString(KEY_RECENT_EXIT_IDS, null)
            ?.split(RECENT_DELIMITER)
            ?.map { it.trim() }
            ?.filter { it.isNotEmpty() }
            ?: emptyList()

    fun setIpv6Enabled(enabled: Boolean) {
        prefs.edit().putBoolean(KEY_IPV6_ENABLED, enabled).apply()
        _ipv6Enabled.value = enabled
    }

    fun setLockdownMode(enabled: Boolean) {
        prefs.edit().putBoolean(KEY_LOCKDOWN_MODE, enabled).apply()
        _lockdownMode.value = enabled
    }

    fun setAllowLan(enabled: Boolean) {
        prefs.edit().putBoolean(KEY_ALLOW_LAN, enabled).apply()
        _allowLan.value = enabled
    }

    fun setDnsState(state: String) {
        val normalized = if (state == DNS_STATE_CUSTOM) DNS_STATE_CUSTOM else DNS_STATE_DEFAULT
        prefs.edit().putString(KEY_DNS_STATE, normalized).apply()
        _dnsState.value = normalized
    }

    /** Replace the custom DNS resolver list. Blank entries are dropped. */
    fun setCustomDnsServers(servers: List<String>) {
        val cleaned = servers.map { it.trim() }.filter { it.isNotEmpty() }
        prefs.edit().putString(KEY_DNS_CUSTOM_SERVERS, cleaned.joinToString(DNS_SERVER_DELIMITER)).apply()
        _customDnsServers.value = cleaned
    }

    fun setBlockAds(enabled: Boolean) {
        prefs.edit().putBoolean(KEY_DNS_BLOCK_ADS, enabled).apply()
        _blockAds.value = enabled
    }

    fun setBlockTrackers(enabled: Boolean) {
        prefs.edit().putBoolean(KEY_DNS_BLOCK_TRACKERS, enabled).apply()
        _blockTrackers.value = enabled
    }

    fun setBlockMalware(enabled: Boolean) {
        prefs.edit().putBoolean(KEY_DNS_BLOCK_MALWARE, enabled).apply()
        _blockMalware.value = enabled
    }

    fun setBlockAdultContent(enabled: Boolean) {
        prefs.edit().putBoolean(KEY_DNS_BLOCK_ADULT, enabled).apply()
        _blockAdultContent.value = enabled
    }

    fun setBlockGambling(enabled: Boolean) {
        prefs.edit().putBoolean(KEY_DNS_BLOCK_GAMBLING, enabled).apply()
        _blockGambling.value = enabled
    }

    fun setBlockSocialMedia(enabled: Boolean) {
        prefs.edit().putBoolean(KEY_DNS_BLOCK_SOCIAL, enabled).apply()
        _blockSocialMedia.value = enabled
    }

    private fun readCustomDnsServers(): List<String> =
        prefs.getString(KEY_DNS_CUSTOM_SERVERS, null)
            ?.split(DNS_SERVER_DELIMITER)
            ?.map { it.trim() }
            ?.filter { it.isNotEmpty() }
            ?: emptyList()

    companion object {
        const val DNS_STATE_DEFAULT = "default"
        const val DNS_STATE_CUSTOM = "custom"
        const val NAT_PMP_PROTOCOL_UDP = "udp"
        const val NAT_PMP_PROTOCOL_TCP = "tcp"
        const val NAT_PMP_DEFAULT_LIFETIME_SECS = 3600
        const val NAT_PMP_MIN_LIFETIME_SECS = 60
        const val NAT_PMP_MAX_LIFETIME_SECS = 86_400

        private const val PREFS_NAME = "warren_local_settings"
        private const val KEY_DAITA_ENABLED = "daita_enabled"
        private const val KEY_NAT_PMP_ENABLED = "nat_pmp_enabled"
        private const val KEY_NAT_PMP_PROTOCOL = "nat_pmp_protocol"
        private const val KEY_NAT_PMP_EXTERNAL_PORT = "nat_pmp_external_port"
        private const val KEY_NAT_PMP_LIFETIME_SECS = "nat_pmp_lifetime_secs"
        private const val KEY_MULTI_HOP_ENABLED = "multi_hop_enabled"
        private const val KEY_ENTRY_COUNTRY = "entry_country"
        private const val KEY_EXIT_COUNTRY = "exit_country"
        private const val KEY_OBFUSCATION_M40 = "obfuscation_m40"
        private const val KEY_SELECTED_EXIT_ID = "selected_exit_id"
        private const val KEY_RECENT_EXIT_IDS = "recent_exit_ids"
        private const val RECENT_DELIMITER = ","
        private const val MAX_RECENT_EXITS = 5
        private const val KEY_IPV6_ENABLED = "ipv6_enabled"
        private const val KEY_LOCKDOWN_MODE = "lockdown_mode"
    private const val KEY_ALLOW_LAN = "allow_lan"
        private const val KEY_DNS_STATE = "dns_state"
        private const val KEY_DNS_CUSTOM_SERVERS = "dns_custom_servers"
        private const val KEY_DNS_BLOCK_ADS = "dns_block_ads"
        private const val KEY_DNS_BLOCK_TRACKERS = "dns_block_trackers"
        private const val KEY_DNS_BLOCK_MALWARE = "dns_block_malware"
        private const val KEY_DNS_BLOCK_ADULT = "dns_block_adult"
        private const val KEY_DNS_BLOCK_GAMBLING = "dns_block_gambling"
        private const val KEY_DNS_BLOCK_SOCIAL = "dns_block_social"
        private const val DNS_SERVER_DELIMITER = ","
    }
}
