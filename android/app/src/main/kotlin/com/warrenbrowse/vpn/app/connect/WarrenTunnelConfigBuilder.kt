package com.warrenbrowse.vpn.app.connect

import co.touchlab.kermit.Logger
import com.warrenbrowse.vpn.app.service.WarrenTunnelConfig
import com.warrenbrowse.vpn.lib.model.wallet.WalletAddress
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository

/**
 * Composes a [WarrenTunnelConfig] from in-memory state.
 *
 * Reads the user's toggles from [WarrenLocalSettingsRepository] and
 * resolves the actual exit / entry relays via [RelayCatalog] (which is
 * itself backed by `WarrenJni.listRelays()`). The picker UI (D.6) will
 * eventually let the user pick an exit by `exit_id` and persist that
 * choice; until then the builder falls back to the first active entry
 * in the catalogue, so the connect flow keeps working without a
 * picker.
 */
class WarrenTunnelConfigBuilder(
    private val localSettings: WarrenLocalSettingsRepository,
    private val relayCatalog: RelayCatalog,
) {

    /**
     * Build a config or `null` if the relay catalogue is empty (no
     * available exit). Callers should surface a "no exit reachable"
     * message to the user when this happens.
     */
    fun build(walletPubkey: WalletAddress): WarrenTunnelConfig? {
        val daitaEnabled = localSettings.daitaEnabled.value
        val natPmpEnabled = localSettings.natPmpEnabled.value
        val multiHopEnabled = localSettings.multiHopEnabled.value
        val obfuscationM40 = localSettings.obfuscationM40.value
        val ipv6Enabled = localSettings.ipv6Enabled.value
        val lockdownMode = localSettings.lockdownMode.value
        val allowLan = localSettings.allowLan.value

        val relays = relayCatalog.listRelays()
        val selectedExitId = localSettings.selectedExitId.value
        val exitCountry = localSettings.exitCountry.value
        val entryCountry = localSettings.entryCountry.value

        // Exit precedence: explicit picker > preferred country > first active.
        val exit = relays
            .firstOrNull { it.active && it.exitId == selectedExitId }
            ?: exitCountry?.let { c -> relays.firstOrNull { it.active && it.country.equals(c, ignoreCase = true) } }
            ?: relays.firstOrNull { it.active }
            ?: run {
                Logger.e("WarrenTunnelConfigBuilder: no active relay in catalogue")
                return null
            }

        // Multi-hop picks a distinct entry relay (different exit_id than the
        // chosen exit). With a single-entry catalogue there is no distinct
        // entry to pick, so the multi-hop toggle is honoured by sending the
        // same relay as entry; the exit still negotiates the multi-hop hop the
        // same way.
        val entryRelay = if (multiHopEnabled) {
            val distinct = relays.filter { it.active && it.exitId != exit.exitId }
            entryCountry?.let { c -> distinct.firstOrNull { it.country.equals(c, ignoreCase = true) } }
                ?: distinct.firstOrNull()
                ?: exit
        } else null

        return WarrenTunnelConfig(
            exitPubkeyHex = exit.exitPubkeyHex,
            exitEndpoint = exit.endpoint,
            walletPubkeyHex = walletPubkey.value,
            entryHop = entryRelay?.let { hop ->
                WarrenTunnelConfig.EntryHop(
                    relayPubkeyHex = hop.exitPubkeyHex,
                    relayEndpoint = hop.endpoint,
                )
            },
            daita = if (daitaEnabled) {
                WarrenTunnelConfig.DaitaSpec(
                    paddingMachine = DEFAULT_DAITA_MACHINE,
                    normalizePackets = true,
                )
            } else null,
            bypassCidrs = emptyList(),
            natPmpEnabled = natPmpEnabled,
            natPmpProtocol = localSettings.natPmpProtocol.value,
            natPmpExternalPort = localSettings.natPmpExternalPort.value,
            natPmpLifetimeSecs = localSettings.natPmpLifetimeSecs.value,
            obfuscationM40 = obfuscationM40,
            enableIpv6 = ipv6Enabled,
            lockdownMode = lockdownMode,
            dns = buildDnsConfig(),
            allowLan = allowLan,
        )
    }

    /**
     * Compose the [WarrenTunnelConfig.DnsConfig] from the persisted DNS
     * settings. Returns `null` when DNS is in default mode with no content
     * blocking, so the tunnel uses the exit forwarder with no extra payload.
     */
    private fun buildDnsConfig(): WarrenTunnelConfig.DnsConfig? {
        val state = localSettings.dnsState.value
        val custom = localSettings.customDnsServers.value
        val blockAds = localSettings.blockAds.value
        val blockTrackers = localSettings.blockTrackers.value
        val blockMalware = localSettings.blockMalware.value
        val blockAdult = localSettings.blockAdultContent.value
        val blockGambling = localSettings.blockGambling.value
        val blockSocial = localSettings.blockSocialMedia.value

        val isCustom = state == WarrenLocalSettingsRepository.DNS_STATE_CUSTOM
        val anyBlocking =
            blockAds || blockTrackers || blockMalware || blockAdult || blockGambling || blockSocial

        if (!isCustom && !anyBlocking) return null

        return WarrenTunnelConfig.DnsConfig(
            state = if (isCustom) {
                WarrenTunnelConfig.DnsConfig.STATE_CUSTOM
            } else {
                WarrenTunnelConfig.DnsConfig.STATE_DEFAULT
            },
            customServers = if (isCustom) custom else emptyList(),
            blockAds = blockAds,
            blockTrackers = blockTrackers,
            blockMalware = blockMalware,
            blockAdultContent = blockAdult,
            blockGambling = blockGambling,
            blockSocialMedia = blockSocial,
        )
    }

    private companion object {
        // Tamaraw is the single padding machine warren-core ships today.
        const val DEFAULT_DAITA_MACHINE = "tamaraw"
    }
}
