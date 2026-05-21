package com.warrenbrowse.vpn.app.connect

import co.touchlab.kermit.Logger
import com.warrenbrowse.vpn.app.service.WarrenTunnelConfig
import com.warrenbrowse.vpn.lib.model.wallet.WalletPubkeyHex
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
    fun build(walletPubkey: WalletPubkeyHex): WarrenTunnelConfig? {
        val daitaEnabled = localSettings.daitaEnabled.value
        val natPmpEnabled = localSettings.natPmpEnabled.value
        val multiHopEnabled = localSettings.multiHopEnabled.value
        val obfuscationM40 = localSettings.obfuscationM40.value

        val relays = relayCatalog.listRelays()
        val selectedExitId = localSettings.selectedExitId.value
        val exit = relays
            .firstOrNull { it.active && it.exitId == selectedExitId }
            ?: relays.firstOrNull { it.active }
            ?: run {
                Logger.e("WarrenTunnelConfigBuilder: no active relay in catalogue")
                return null
            }

        // D.4 step 17 follow-up : multi-hop picks a distinct entry relay
        // (different exit_id than the chosen exit). With a single-entry
        // catalogue today there is no distinct entry to pick, so the
        // multi-hop toggle is honoured by sending the same relay as
        // entry - the exit still negotiates the multi-hop hop the same
        // way; the picker UI will replace this fall-back.
        val entryRelay = if (multiHopEnabled) {
            relays.firstOrNull { it.active && it.exitId != exit.exitId } ?: exit
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
            obfuscationM40 = obfuscationM40,
        )
    }

    private companion object {
        // Tamaraw is the single padding machine warren-core ships today.
        const val DEFAULT_DAITA_MACHINE = "tamaraw"
    }
}
