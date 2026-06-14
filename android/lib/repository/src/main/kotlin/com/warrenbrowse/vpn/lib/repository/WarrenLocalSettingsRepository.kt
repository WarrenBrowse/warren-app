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

    /**
     * Whether the first-launch onboarding wizard has been completed. Gates
     * the welcome flow so it is shown once (to new users, before wallet
     * creation). Defaults to false.
     */
    private val _onboardingCompleted = MutableStateFlow(prefs.getBoolean(KEY_ONBOARDING_DONE, false))
    val onboardingCompleted: StateFlow<Boolean> = _onboardingCompleted.asStateFlow()

    /** TUN interface MTU (clamped to [MTU_MIN]..[MTU_MAX]). */
    private val _tunnelMtu = MutableStateFlow(prefs.getInt(KEY_TUNNEL_MTU, MTU_MAX))
    val tunnelMtu: StateFlow<Int> = _tunnelMtu.asStateFlow()

    /**
     * Last-known subscription expiry (Unix epoch seconds; 0 = unknown).
     * Cached from the on-demand `/v1/subscription` fetch + voucher redeem so
     * the UI can surface the subscription status (and a near-expiry warning)
     * without a fresh biometric-gated request on every screen open.
     */
    private val _cachedSubscriptionExpiry = MutableStateFlow(prefs.getLong(KEY_SUBSCRIPTION_EXPIRY, 0L))
    val cachedSubscriptionExpiry: StateFlow<Long> = _cachedSubscriptionExpiry.asStateFlow()

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

    /**
     * Whether recently-used exits are remembered. When off, no new recents
     * are recorded and the existing list is cleared (desktop "recents"
     * privacy toggle parity). Defaults to on.
     */
    private val _recentsEnabled = MutableStateFlow(prefs.getBoolean(KEY_RECENTS_ENABLED, true))
    val recentsEnabled: StateFlow<Boolean> = _recentsEnabled.asStateFlow()

    /**
     * User-defined custom lists of exits (desktop "custom lists" parity):
     * name -> ordered exit ids. Surfaced at the top of the location picker so
     * users can group favourite exits. Persisted as a [String] set of names
     * plus one delimited string per list, so no serialization plugin is
     * needed in this module.
     */
    private val _customLists = MutableStateFlow(readCustomLists())
    val customLists: StateFlow<Map<String, List<String>>> = _customLists.asStateFlow()

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
        if (!_recentsEnabled.value) return
        val updated = (listOf(exitId) + _recentExitIds.value.filter { it != exitId })
            .take(MAX_RECENT_EXITS)
        prefs.edit().putString(KEY_RECENT_EXIT_IDS, updated.joinToString(RECENT_DELIMITER)).apply()
        _recentExitIds.value = updated
    }

    /** Forget all recently-used exits. */
    fun clearRecentExits() {
        prefs.edit().remove(KEY_RECENT_EXIT_IDS).apply()
        _recentExitIds.value = emptyList()
    }

    /**
     * Enable/disable remembering recently-used exits. Turning it off also
     * forgets the current list so the privacy choice takes effect immediately.
     */
    fun setRecentsEnabled(enabled: Boolean) {
        prefs.edit().putBoolean(KEY_RECENTS_ENABLED, enabled).apply()
        _recentsEnabled.value = enabled
        if (!enabled) clearRecentExits()
    }

    private fun readRecentExitIds(): List<String> =
        prefs.getString(KEY_RECENT_EXIT_IDS, null)
            ?.split(RECENT_DELIMITER)
            ?.map { it.trim() }
            ?.filter { it.isNotEmpty() }
            ?: emptyList()

    /** Create an empty custom list. No-op if blank or already present. */
    fun createCustomList(name: String) {
        val trimmed = name.trim()
        if (trimmed.isEmpty()) return
        val names = customListNames()
        if (!names.add(trimmed)) return
        prefs.edit().putStringSet(KEY_CUSTOM_LIST_NAMES, names).apply()
        _customLists.value = readCustomLists()
    }

    /** Delete a custom list and its members. No-op if the list is unknown. */
    fun deleteCustomList(name: String) {
        val names = customListNames()
        if (!names.remove(name)) return
        prefs.edit()
            .putStringSet(KEY_CUSTOM_LIST_NAMES, names)
            .remove(customListKey(name))
            .apply()
        _customLists.value = readCustomLists()
    }

    /**
     * Add an exit to a custom list (creating the list if needed), keeping
     * insertion order and de-duplicating.
     */
    fun addExitToCustomList(name: String, exitId: String) {
        val trimmed = name.trim()
        if (trimmed.isEmpty() || exitId.isEmpty()) return
        val names = customListNames()
        names.add(trimmed)
        val current = _customLists.value[trimmed].orEmpty()
        if (exitId in current) return
        val updated = current + exitId
        prefs.edit()
            .putStringSet(KEY_CUSTOM_LIST_NAMES, names)
            .putString(customListKey(trimmed), updated.joinToString(RECENT_DELIMITER))
            .apply()
        _customLists.value = readCustomLists()
    }

    /** Remove an exit from a custom list. No-op if absent. */
    fun removeExitFromCustomList(name: String, exitId: String) {
        val current = _customLists.value[name] ?: return
        if (exitId !in current) return
        val updated = current.filter { it != exitId }
        prefs.edit()
            .putString(customListKey(name), updated.joinToString(RECENT_DELIMITER))
            .apply()
        _customLists.value = readCustomLists()
    }

    // Defensive copy: the set returned by getStringSet must not be mutated.
    private fun customListNames(): MutableSet<String> =
        prefs.getStringSet(KEY_CUSTOM_LIST_NAMES, emptySet()).orEmpty().toMutableSet()

    private fun customListKey(name: String): String = KEY_CUSTOM_LIST_PREFIX + name

    private fun readCustomLists(): Map<String, List<String>> =
        customListNames().sorted().associateWith { name ->
            prefs.getString(customListKey(name), null)
                ?.split(RECENT_DELIMITER)
                ?.map { it.trim() }
                ?.filter { it.isNotEmpty() }
                ?: emptyList()
        }

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

    /** Mark the first-launch onboarding wizard as completed (shown once). */
    fun setOnboardingCompleted(done: Boolean) {
        prefs.edit().putBoolean(KEY_ONBOARDING_DONE, done).apply()
        _onboardingCompleted.value = done
    }

    /** Set the TUN MTU, clamped to [MTU_MIN]..[MTU_MAX]. */
    fun setTunnelMtu(mtu: Int) {
        val clamped = mtu.coerceIn(MTU_MIN, MTU_MAX)
        prefs.edit().putInt(KEY_TUNNEL_MTU, clamped).apply()
        _tunnelMtu.value = clamped
    }

    /** Cache the last-known subscription expiry (Unix epoch seconds). */
    fun setCachedSubscriptionExpiry(expiryUnixSecs: Long) {
        prefs.edit().putLong(KEY_SUBSCRIPTION_EXPIRY, expiryUnixSecs).apply()
        _cachedSubscriptionExpiry.value = expiryUnixSecs
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
        private const val KEY_SELECTED_EXIT_ID = "selected_exit_id"
        private const val KEY_RECENT_EXIT_IDS = "recent_exit_ids"
        private const val KEY_RECENTS_ENABLED = "recents_enabled"
        private const val RECENT_DELIMITER = ","
        private const val KEY_CUSTOM_LIST_NAMES = "custom_list_names"
        private const val KEY_CUSTOM_LIST_PREFIX = "custom_list_exits_"
        private const val MAX_RECENT_EXITS = 5
        private const val KEY_IPV6_ENABLED = "ipv6_enabled"
        private const val KEY_LOCKDOWN_MODE = "lockdown_mode"
    private const val KEY_ALLOW_LAN = "allow_lan"
    private const val KEY_ONBOARDING_DONE = "onboarding_completed"
    private const val KEY_TUNNEL_MTU = "tunnel_mtu"
    const val MTU_MIN = 576
    const val MTU_MAX = 1280
    private const val KEY_SUBSCRIPTION_EXPIRY = "subscription_expiry_unix_secs"
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
