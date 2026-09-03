package com.warrenbrowse.vpn.app.connect

import co.touchlab.kermit.Logger
import com.warrenbrowse.vpn.jni.WarrenJni
import com.warrenbrowse.vpn.jni.WarrenNativeRuntime
import com.warrenbrowse.vpn.app.service.WarrenTunnelConfig
import com.warrenbrowse.vpn.lib.model.wallet.WalletAddress
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository
import com.warrenbrowse.vpn.lib.repository.WarrenProductFlags
import com.warrenbrowse.vpn.lib.repository.resolveExitPin
import com.warrenbrowse.vpn.lib.repository.resolveFailoverExit

/**
 * Composes a [WarrenTunnelConfig] from in-memory state.
 *
 * Reads the user's toggles from [WarrenLocalSettingsRepository] and
 * resolves the actual exit / entry relays via [RelayCatalog] (which is
 * itself backed by `WarrenJni.listRelays()`, on the daemon's hourly
 * refresh cadence). The picker pin can name a
 * country, a city or one exit, so it is resolved here to the single
 * concrete exit the engine dials; with nothing pinned (or nothing active
 * in the pinned scope) the builder falls back to the preferred exit
 * country, then to the first active entry in the catalogue.
 */
class WarrenTunnelConfigBuilder(
    private val localSettings: WarrenLocalSettingsRepository,
    private val productFlags: WarrenProductFlags,
    private val relayCatalog: RelayCatalog,
    // Injectable so the builder stays unit-testable without the native lib.
    // Production binds the JNI prefetch (signed multi-hop directory, fetched on
    // the physical network before the TUN is up); tests pass a stub. Wrapped in
    // a lambda (not a `::` reference) so constructing the builder never triggers
    // WarrenJni's static loadLibrary in a JVM test.
    private val fetchMultihopDirectory: () -> String = {
        WarrenNativeRuntime.awaitReadyBlocking()
        WarrenJni.fetchMultihopDirectory()
    },
) {

    /**
     * Build a config or `null` if the relay catalogue is empty (no
     * available exit). Callers should surface a "no exit reachable"
     * message to the user when this happens.
     *
     * [excludedExitPubkeyHex] names the exit an automatic retry is failing
     * over from: the pick then avoids it whenever the pin leaves an
     * alternative (desktop `assemble_failover_for_attempt`), and degrades to
     * the ordinary pick when it does not, so a transient refusal never
     * strands the user without an exit.
     */
    fun build(
        walletPubkey: WalletAddress,
        excludedExitPubkeyHex: String? = null,
    ): WarrenTunnelConfig? {
        val daitaEnabled = localSettings.daitaEnabled.value
        val natPmpEnabled = localSettings.natPmpEnabled.value
        val ipv6Enabled = localSettings.ipv6Enabled.value
        val lockdownMode = localSettings.lockdownMode.value
        val allowLan = localSettings.allowLan.value

        // The fresh snapshot, or a fetch when it is stale: an exit switch dials
        // from the list the user just picked from instead of refetching it.
        val relays = relayCatalog.relaysForDial()
        val exitCountry = localSettings.exitCountry.value
        val multiHopEnabled = localSettings.multiHopEnabled.value

        // Exit precedence: failover alternative > explicit picker > preferred
        // country > first active. The picker pin can name a country or a city,
        // so it is resolved to one concrete exit here: the engine only ever
        // accepts a single exit.
        val pin = localSettings.exitPin.value
        val alternative =
            excludedExitPubkeyHex?.let { resolveFailoverExit(pin, exitCountry, relays, it) }
        val exit = alternative
            ?: resolveExitPin(pin, relays)
            ?: exitCountry?.let { c -> relays.firstOrNull { it.active && it.country.equals(c, ignoreCase = true) } }
            ?: relays.firstOrNull { it.active }
            ?: run {
                Logger.e("WarrenTunnelConfigBuilder: no active relay in catalogue")
                return null
            }

        // Always ride the multi-hop wire: the production exit fleet runs the
        // unified `:443` dispatcher (`warren-exit --multihop`), which ONLY
        // accepts a `WarrenMultihopFrame` (exit_id + setup) as the first frame.
        // A bare `Setup` (no `WarrenMultihopFrame` wrapper) cannot be decoded
        // by the dispatcher and is silently dropped (the client then sees
        // "read SetupAck: connection lost"). So we always send a
        // present-but-empty `entry_hop` to route warren-jni through
        // `run_multi_hop_session`. Whether that yields TWO hops or ONE is the
        // user's choice: `multihopTwoHop` from the multi-hop toggle. On
        // `true` warren-jni auto-selects a DISTINCT entry relay (HPKE entry
        // then exit, real 2-hop); on `false` it collapses the circuit onto the
        // exit node itself (a 1-hop circuit on the same wire, faster but the
        // node sees both the user IP and the destination). Default `true`
        // preserves today's Android behavior.
        // Prefetch the signed multi-hop directory here, on the physical network,
        // BEFORE the VpnService TUN is established. warren-jni verifies + uses
        // this blob in run_multi_hop_session; fetching it there (post-TUN) would
        // route the request into the half-open tunnel and blackhole it.
        val multihopDirectory = fetchMultihopDirectory()
        if (multihopDirectory.isEmpty()) {
            Logger.e("WarrenTunnelConfigBuilder: multi-hop directory fetch returned empty")
            return null
        }

        return WarrenTunnelConfig(
            exitPubkeyHex = exit.exitPubkeyHex,
            exitEndpoint = exit.endpoint,
            exitId = exit.exitId,
            walletPubkeyHex = walletPubkey.value,
            entryHop = WarrenTunnelConfig.EntryHop(),
            // Preferred entry country (null = auto). Honoured natively by
            // run_multi_hop_session once the .so carries the entry_country field.
            entryCountry = localSettings.entryCountry.value,
            multihopTwoHop = multiHopEnabled,
            multihopDirectoryRaw = multihopDirectory,
            daita = if (daitaEnabled) {
                WarrenTunnelConfig.DaitaSpec(
                    paddingMachine = DEFAULT_DAITA_MACHINE,
                    normalizePackets = true,
                )
            } else null,
            natPmpEnabled = natPmpEnabled,
            natPmpProtocol = localSettings.natPmpProtocol.value,
            natPmpExternalPort = localSettings.natPmpExternalPort.value,
            natPmpLifetimeSecs = localSettings.natPmpLifetimeSecs.value,
            enableIpv6 = ipv6Enabled,
            lockdownMode = lockdownMode,
            dns = buildDnsConfig(),
            allowLan = allowLan,
            // Beta builds never send a user bandwidth value: the beta cap
            // is network-imposed and enforced by the exits.
            maxRateBps = if (productFlags.isBeta) 0 else localSettings.maxRateBps.value,
        )
    }

    /**
     * Compose the [WarrenTunnelConfig.DnsConfig] from the persisted DNS
     * settings. Returns `null` when DNS is in default mode with no content
     * blocking, so the tunnel uses the exit resolver with no extra payload.
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
