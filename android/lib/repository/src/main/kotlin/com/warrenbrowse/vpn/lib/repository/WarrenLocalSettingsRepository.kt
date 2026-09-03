package com.warrenbrowse.vpn.lib.repository

import android.content.Context
import android.content.SharedPreferences
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/** Trust-on-first-use verdict for an exit's pinned public key. */
sealed interface ExitKeyVerdict {
    /** No key pinned yet for this exit; the caller should pin the current one. */
    data object FirstSeen : ExitKeyVerdict

    /** The presented key matches the pin. */
    data object Match : ExitKeyVerdict

    /** The presented key differs from the [pinned] one; fail closed. */
    data class Mismatch(val pinned: String) : ExitKeyVerdict
}

/**
 * Warren-side tunnel settings, persisted via [SharedPreferences].
 *
 * Kept separate from the legacy proto-backed [UserPreferencesRepository]
 * for two reasons:
 *   1. Mullvad's protobuf schema cannot grow without coordinated
 *      migration of the dead `mullvad-daemon` consumers.
 *   2. Warren's set of toggles (DAITA on/off, NAT-PMP on/off,
 *      multi-hop entry hop pubkey, obfuscation flag, bypass CIDRs)
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

    // Multi-hop topology toggle. Default ON (2-hop): a distinct entry relay
    // fronts the exit so no single node sees both the user IP and the
    // destination. OFF selects the single-hop 1-hop circuit (one node, faster).
    // Defaulting ON preserves the topology existing users already connect with;
    // single-hop is strictly opt-in (WarrenTunnelConfigBuilder reads this into
    // `multihopTwoHop`).
    private val _multiHopEnabled = MutableStateFlow(prefs.getBoolean(KEY_MULTI_HOP_ENABLED, true))
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

    /** Split tunnelling master switch: when off, [excludedApps] is ignored. */
    private val _splitTunnelingEnabled =
        MutableStateFlow(prefs.getBoolean(KEY_SPLIT_TUNNELING_ENABLED, false))
    val splitTunnelingEnabled: StateFlow<Boolean> = _splitTunnelingEnabled.asStateFlow()

    /**
     * Package names routed OUTSIDE the tunnel (VpnService.Builder
     * `addDisallowedApplication`). Applied only while [splitTunnelingEnabled].
     */
    private val _excludedApps =
        MutableStateFlow(prefs.getStringSet(KEY_EXCLUDED_APPS, emptySet())?.toSet() ?: emptySet())
    val excludedApps: StateFlow<Set<String>> = _excludedApps.asStateFlow()

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
     * Client-side bandwidth ceiling in bits per second, enforced on both
     * tunnel directions independently at the datapath. 0 = unlimited.
     * Applied at tunnel start (a change takes effect on the next connect,
     * like the MTU). Beta builds never expose the setter in the UI (the
     * network cap is server-enforced there).
     */
    private val _maxRateBps = MutableStateFlow(prefs.getLong(KEY_MAX_RATE_BPS, 0L))
    val maxRateBps: StateFlow<Long> = _maxRateBps.asStateFlow()

    /**
     * Last-known subscription expiry (Unix epoch seconds; 0 = unknown).
     * Cached from the on-demand `/v1/subscription` fetch + voucher redeem so
     * the UI can surface the subscription status (and a near-expiry warning)
     * without a fresh biometric-gated request on every screen open.
     */
    private val _cachedSubscriptionExpiry = MutableStateFlow(prefs.getLong(KEY_SUBSCRIPTION_EXPIRY, 0L))
    val cachedSubscriptionExpiry: StateFlow<Long> = _cachedSubscriptionExpiry.asStateFlow()

    /**
     * Last-known network bandwidth cap in bits per second, or `null` when the
     * `/v1/network` feed has never answered on this install. `0` is a real
     * answer meaning "no cap".
     *
     * Cached so a cold start renders the final beta-badge copy from the first
     * frame: the live feed is a network round trip, and rendering the
     * cap-unknown wording first swapped the line under the user's eyes.
     */
    private val _cachedNetworkRateBps =
        MutableStateFlow(prefs.getLong(KEY_CACHED_NETWORK_RATE_BPS, RATE_NEVER_FETCHED)
            .takeIf { it != RATE_NEVER_FETCHED })
    val cachedNetworkRateBps: StateFlow<Long?> = _cachedNetworkRateBps.asStateFlow()

    /**
     * Display name last resolved for an [ExitPin.Exit] selection, or `null`
     * when none is known.
     *
     * The pin stores an exit id, and only the relay catalogue can turn that
     * into a city; the catalogue is fetched over the network, so at cold start
     * the switch-location button had nothing to show and named "Automatic",
     * contradicting the user's own selection until the fetch landed. Carries
     * the exit id it was resolved for, so the label of a previous selection is
     * never shown for the current one.
     */
    private val _exitPinLabel = MutableStateFlow(readExitPinLabel())
    val exitPinLabel: StateFlow<ExitPinLabel?> = _exitPinLabel.asStateFlow()

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
     * Wired by the location picker UI.
     */
    private val _selectedExitId = MutableStateFlow(prefs.getString(KEY_SELECTED_EXIT_ID, null))
    val selectedExitId: StateFlow<String?> = _selectedExitId.asStateFlow()

    /**
     * The user's location-picker selection at whatever geographical depth they
     * chose it (desktop parity: a country, a city or one exit). Resolved to a
     * concrete exit at dial time by [resolveExitPin].
     *
     * [selectedExitId] stays the single-exit projection of this, so a pin at a
     * broader depth reads as `null` there and the existing fallback chain runs.
     */
    private val _exitPin = MutableStateFlow(readExitPin())
    val exitPin: StateFlow<ExitPin> = _exitPin.asStateFlow()

    /**
     * Recently selected locations at the depth they were picked (a country, a
     * city or one exit), most-recent-first, capped at [MAX_RECENT_EXITS].
     * Surfaced at the top of the location picker so frequently-used locations
     * are one tap away. Desktop records the location constraint itself
     * (`Recent::try_from(&RelaySettings)`), so a country pick is a recent in
     * its own right, never flattened to the exit it resolved to.
     */
    private val _recentPins = MutableStateFlow(readRecentPins())
    val recentPins: StateFlow<List<ExitPin>> = _recentPins.asStateFlow()

    /**
     * Whether recently-used exits are remembered. When off, no new recents
     * are recorded and the existing list is cleared (desktop "recents"
     * privacy toggle parity). Defaults to on.
     */
    private val _recentsEnabled = MutableStateFlow(prefs.getBoolean(KEY_RECENTS_ENABLED, true))
    val recentsEnabled: StateFlow<Boolean> = _recentsEnabled.asStateFlow()

    /**
     * The desktop `forumNotifications` GUI setting: whether community-forum
     * activity is shown at all (the header bell, the local notification, the
     * launcher badge). On for an install that never touched it.
     */
    private val _forumNotificationsEnabled =
        MutableStateFlow(prefs.getBoolean(KEY_FORUM_NOTIFICATIONS_ENABLED, true))
    val forumNotificationsEnabled: StateFlow<Boolean> = _forumNotificationsEnabled.asStateFlow()

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

    fun setSplitTunnelingEnabled(enabled: Boolean) {
        prefs.edit().putBoolean(KEY_SPLIT_TUNNELING_ENABLED, enabled).apply()
        _splitTunnelingEnabled.value = enabled
    }

    fun addExcludedApp(packageName: String) = updateExcludedApps { it + packageName }

    fun removeExcludedApp(packageName: String) = updateExcludedApps { it - packageName }

    private fun updateExcludedApps(transform: (Set<String>) -> Set<String>) {
        val updated = transform(_excludedApps.value)
        // Store a fresh set: SharedPreferences keeps a reference to the passed
        // set and its own getStringSet return value must not be mutated.
        prefs.edit().putStringSet(KEY_EXCLUDED_APPS, HashSet(updated)).apply()
        _excludedApps.value = updated
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
        setExitPin(if (exitId == null) ExitPin.Automatic else ExitPin.Exit(exitId))
    }

    /**
     * Persist the user's location-picker selection at any depth. The two
     * persisted keys are mutually exclusive by construction: writing one always
     * removes the other, so a stored selection can never be ambiguous.
     */
    fun setExitPin(pin: ExitPin) {
        val editor = prefs.edit()
        when (pin) {
            ExitPin.Automatic -> editor.remove(KEY_SELECTED_EXIT_ID).remove(KEY_SELECTED_SCOPE)
            is ExitPin.Exit ->
                editor.putString(KEY_SELECTED_EXIT_ID, pin.exitId).remove(KEY_SELECTED_SCOPE)
            is ExitPin.Country ->
                editor.remove(KEY_SELECTED_EXIT_ID).putString(KEY_SELECTED_SCOPE, pin.country)
            is ExitPin.City ->
                editor
                    .remove(KEY_SELECTED_EXIT_ID)
                    .putString(KEY_SELECTED_SCOPE, pin.country + SCOPE_DELIMITER + pin.city)
        }
        editor.apply()
        _exitPin.value = pin
        _selectedExitId.value = (pin as? ExitPin.Exit)?.exitId
        // A broader pin renders its own name, so a leftover exit label would
        // contradict it; the new exit's label is written once it resolves.
        if (pin !is ExitPin.Exit) clearExitPinLabel()
        // Selecting (not clearing) a location records it as recently used.
        recordRecentPin(pin)
    }

    /**
     * Read the stored pin. The single-exit key is read first so a selection
     * written by a build that predates the scoped pin keeps working untouched.
     */
    private fun readExitPin(): ExitPin {
        val exitId = prefs.getString(KEY_SELECTED_EXIT_ID, null)?.takeIf { it.isNotEmpty() }
        val scope = prefs.getString(KEY_SELECTED_SCOPE, null)?.takeIf { it.isNotEmpty() }
        return when {
            exitId != null -> ExitPin.Exit(exitId)
            scope == null -> ExitPin.Automatic
            !scope.contains(SCOPE_DELIMITER) -> ExitPin.Country(scope)
            else ->
                ExitPin.City(
                    scope.substringBefore(SCOPE_DELIMITER),
                    scope.substringAfter(SCOPE_DELIMITER),
                )
        }
    }

    /** Push [pin] to the front of the recents list (deduped, capped); Automatic pins nothing. */
    fun recordRecentPin(pin: ExitPin) {
        if (!_recentsEnabled.value || pin == ExitPin.Automatic) return
        val updated = (listOf(pin) + _recentPins.value.filter { it != pin }).take(MAX_RECENT_EXITS)
        prefs.edit()
            .putString(KEY_RECENT_PINS, updated.joinToString(RECENT_DELIMITER) { encodeRecentPin(it) })
            .remove(KEY_RECENT_EXIT_IDS)
            .apply()
        _recentPins.value = updated
    }

    /** Forget all recently-used locations. */
    fun clearRecents() {
        prefs.edit().remove(KEY_RECENT_PINS).remove(KEY_RECENT_EXIT_IDS).apply()
        _recentPins.value = emptyList()
    }

    fun setForumNotificationsEnabled(enabled: Boolean) {
        prefs.edit().putBoolean(KEY_FORUM_NOTIFICATIONS_ENABLED, enabled).apply()
        _forumNotificationsEnabled.value = enabled
    }

    /**
     * Enable/disable remembering recently-used locations. Turning it off also
     * forgets the current list so the privacy choice takes effect immediately.
     */
    fun setRecentsEnabled(enabled: Boolean) {
        prefs.edit().putBoolean(KEY_RECENTS_ENABLED, enabled).apply()
        _recentsEnabled.value = enabled
        if (!enabled) clearRecents()
    }

    /**
     * The stored list, or the exit-id list an older build wrote (every entry
     * of which was a single exit) when the scoped key is absent.
     */
    private fun readRecentPins(): List<ExitPin> {
        val scoped = prefs.getString(KEY_RECENT_PINS, null)
        if (scoped != null) {
            return scoped.split(RECENT_DELIMITER).mapNotNull { decodeRecentPin(it.trim()) }
        }
        return prefs.getString(KEY_RECENT_EXIT_IDS, null)
            ?.split(RECENT_DELIMITER)
            ?.map { it.trim() }
            ?.filter { it.isNotEmpty() }
            ?.map { ExitPin.Exit(it) }
            ?: emptyList()
    }

    /** `exit:<id>`, `country:<cc>` or `city:<cc>/<city>`; the depth travels with the entry. */
    private fun encodeRecentPin(pin: ExitPin): String =
        when (pin) {
            ExitPin.Automatic -> ""
            is ExitPin.Exit -> RECENT_EXIT_PREFIX + pin.exitId
            is ExitPin.Country -> RECENT_COUNTRY_PREFIX + pin.country
            is ExitPin.City -> RECENT_CITY_PREFIX + pin.country + SCOPE_DELIMITER + pin.city
        }

    private fun decodeRecentPin(entry: String): ExitPin? =
        when {
            entry.startsWith(RECENT_EXIT_PREFIX) ->
                entry.removePrefix(RECENT_EXIT_PREFIX).takeIf { it.isNotEmpty() }?.let { ExitPin.Exit(it) }
            entry.startsWith(RECENT_COUNTRY_PREFIX) ->
                entry.removePrefix(RECENT_COUNTRY_PREFIX).takeIf { it.isNotEmpty() }?.let { ExitPin.Country(it) }
            entry.startsWith(RECENT_CITY_PREFIX) -> {
                val scope = entry.removePrefix(RECENT_CITY_PREFIX)
                val country = scope.substringBefore(SCOPE_DELIMITER, "")
                val city = scope.substringAfter(SCOPE_DELIMITER, "")
                if (country.isEmpty() || city.isEmpty()) null else ExitPin.City(country, city)
            }
            else -> null
        }

    /**
     * Trust-on-first-use verdict for an exit's public key (desktop
     * "pubkey pinning" parity). The first key seen for an [exitId] is the
     * pin; a later connect with a different key for the same exit is a
     * [ExitKeyVerdict.Mismatch] (operator key rotation or, defence-in-depth,
     * a compromised directory). Callers fail closed on a mismatch.
     */
    fun exitKeyVerdict(exitId: String, pubkeyHex: String): ExitKeyVerdict {
        if (exitId.isEmpty() || pubkeyHex.isEmpty()) return ExitKeyVerdict.Match
        val pinned = prefs.getString(pinKey(exitId), null) ?: return ExitKeyVerdict.FirstSeen
        return if (pinned == pubkeyHex) {
            ExitKeyVerdict.Match
        } else {
            ExitKeyVerdict.Mismatch(pinned)
        }
    }

    /** Pin (or re-pin, on explicit trust) an exit's key. */
    fun trustExitKey(exitId: String, pubkeyHex: String) {
        if (exitId.isEmpty() || pubkeyHex.isEmpty()) return
        val ids = pinnedExitIds()
        ids.add(exitId)
        prefs.edit()
            .putStringSet(KEY_PINNED_EXIT_IDS, ids)
            .putString(pinKey(exitId), pubkeyHex)
            .apply()
    }

    /**
     * Forget every pinned exit key (desktop "Reset pinned exit keys") and
     * return how many entries were dropped. The count is what the caller shows
     * back to the user: this disarms exit-key substitution detection until
     * every exit is re-pinned, so the action must not complete silently.
     */
    fun resetExitKeyPins(): Int {
        val pinned = pinnedExitIds()
        val editor = prefs.edit()
        pinned.forEach { editor.remove(pinKey(it)) }
        editor.remove(KEY_PINNED_EXIT_IDS).apply()
        return pinned.size
    }

    private fun pinnedExitIds(): MutableSet<String> =
        prefs.getStringSet(KEY_PINNED_EXIT_IDS, emptySet()).orEmpty().toMutableSet()

    private fun pinKey(exitId: String): String = KEY_EXIT_PIN_PREFIX + exitId

    /** Create an empty custom list. No-op if blank or already present. */
    fun createCustomList(name: String) {
        val trimmed = name.trim()
        if (trimmed.isEmpty()) return
        val names = customListNames()
        if (!names.add(trimmed)) return
        prefs.edit().putStringSet(KEY_CUSTOM_LIST_NAMES, names).apply()
        _customLists.value = readCustomLists()
    }

    /**
     * Rename a custom list, carrying its members. No-op if [oldName] is unknown,
     * if the new name is blank or unchanged, or if a list already uses the new
     * name (renaming never silently merges or overwrites another list).
     */
    fun renameCustomList(oldName: String, newName: String) {
        val trimmed = newName.trim()
        if (trimmed.isEmpty() || trimmed == oldName) return
        val names = customListNames()
        if (oldName !in names || trimmed in names) return
        val members = prefs.getString(customListKey(oldName), null)
        names.remove(oldName)
        names.add(trimmed)
        val editor =
            prefs.edit()
                .putStringSet(KEY_CUSTOM_LIST_NAMES, names)
                .remove(customListKey(oldName))
        if (members != null) editor.putString(customListKey(trimmed), members)
        editor.apply()
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

    /** Set the bandwidth ceiling in bits per second (0 or negative = unlimited). */
    fun setMaxRateBps(bps: Long) {
        val normalized = bps.coerceAtLeast(0L)
        prefs.edit().putLong(KEY_MAX_RATE_BPS, normalized).apply()
        _maxRateBps.value = normalized
    }

    /** Cache the last-known subscription expiry (Unix epoch seconds). */
    fun setCachedSubscriptionExpiry(expiryUnixSecs: Long) {
        prefs.edit().putLong(KEY_SUBSCRIPTION_EXPIRY, expiryUnixSecs).apply()
        _cachedSubscriptionExpiry.value = expiryUnixSecs
    }

    /** Cache the bandwidth cap the network feed reported (`null` = no cap). */
    fun setCachedNetworkRateBps(bps: Long?) {
        val stored = bps?.coerceAtLeast(0L) ?: 0L
        prefs.edit().putLong(KEY_CACHED_NETWORK_RATE_BPS, stored).apply()
        _cachedNetworkRateBps.value = stored
    }

    /** Remember the display name resolved for [exitId]. */
    fun setExitPinLabel(exitId: String, label: String) {
        val cleanedId = exitId.trim()
        val cleaned = label.trim()
        if (cleanedId.isEmpty() || cleaned.isEmpty()) return
        prefs.edit().putString(KEY_SELECTED_EXIT_LABEL, cleanedId + SCOPE_DELIMITER + cleaned)
            .apply()
        _exitPinLabel.value = ExitPinLabel(cleanedId, cleaned)
    }

    private fun clearExitPinLabel() {
        prefs.edit().remove(KEY_SELECTED_EXIT_LABEL).apply()
        _exitPinLabel.value = null
    }

    private fun readExitPinLabel(): ExitPinLabel? {
        val stored = prefs.getString(KEY_SELECTED_EXIT_LABEL, null).orEmpty()
        val exitId = stored.substringBefore(SCOPE_DELIMITER, "")
        val label = stored.substringAfter(SCOPE_DELIMITER, "")
        return if (exitId.isEmpty() || label.isEmpty()) null else ExitPinLabel(exitId, label)
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
        private const val KEY_SELECTED_SCOPE = "selected_exit_scope"
        private const val KEY_SELECTED_EXIT_LABEL = "selected_exit_label"
        private const val KEY_CACHED_NETWORK_RATE_BPS = "cached_network_rate_bps"

        /** Sentinel for "the network feed has never answered on this install". */
        private const val RATE_NEVER_FETCHED = -1L
        // Country codes are ISO alpha-2 so they never carry this character,
        // which makes the first occurrence the unambiguous country/city split.
        private const val SCOPE_DELIMITER = "/"
        private const val KEY_RECENT_EXIT_IDS = "recent_exit_ids"
        private const val KEY_RECENT_PINS = "recent_pins"
        private const val RECENT_EXIT_PREFIX = "exit:"
        private const val RECENT_COUNTRY_PREFIX = "country:"
        private const val RECENT_CITY_PREFIX = "city:"
        private const val KEY_RECENTS_ENABLED = "recents_enabled"
        private const val KEY_FORUM_NOTIFICATIONS_ENABLED = "forum_notifications_enabled"
        private const val RECENT_DELIMITER = ","
        private const val KEY_CUSTOM_LIST_NAMES = "custom_list_names"
        private const val KEY_CUSTOM_LIST_PREFIX = "custom_list_exits_"
        private const val KEY_PINNED_EXIT_IDS = "pinned_exit_ids"
        private const val KEY_EXIT_PIN_PREFIX = "exit_pin_"
        private const val MAX_RECENT_EXITS = 5
        private const val KEY_IPV6_ENABLED = "ipv6_enabled"
        private const val KEY_LOCKDOWN_MODE = "lockdown_mode"
    private const val KEY_ALLOW_LAN = "allow_lan"
    private const val KEY_SPLIT_TUNNELING_ENABLED = "split_tunneling_enabled"
    private const val KEY_EXCLUDED_APPS = "split_tunneling_excluded_apps"
    private const val KEY_ONBOARDING_DONE = "onboarding_completed"
    private const val KEY_TUNNEL_MTU = "tunnel_mtu"
    const val MTU_MIN = 576
    const val MTU_MAX = 1280
    private const val KEY_MAX_RATE_BPS = "max_rate_bps"
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
