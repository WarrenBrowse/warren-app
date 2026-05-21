package com.warrenbrowse.vpn.app.connect

import com.warrenbrowse.vpn.app.service.WarrenTunnelConfig
import com.warrenbrowse.vpn.lib.model.wallet.WalletPubkeyHex
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository

/**
 * Composes a [WarrenTunnelConfig] from in-memory state.
 *
 * D.4 step 7 cut: the only inputs the wiring needs *right now* are the
 * wallet pubkey (so the exit can authorise the session) and a stable
 * exit identity. Multi-hop entry resolution and DAITA spec selection
 * are not yet wired (D.4 step 8 swaps in the relay-selector for multi-
 * hop and a DAITA picker for the padding-machine selection); the
 * builder currently reads on/off toggles from [WarrenLocalSettingsRepository]
 * and substitutes hardcoded payloads when a toggle is on, so the wire
 * format is exercised end-to-end even before the picker UIs land.
 *
 * For the very first end-to-end smoke we point at warren-exit-1 prod
 * (the same exit the bench has been hitting in Session F-M). Once the
 * relay selector lands on Warren mobile, this builder is replaced by a
 * RelaySelector-driven path.
 */
class WarrenTunnelConfigBuilder(
    private val localSettings: WarrenLocalSettingsRepository,
) {

    fun build(walletPubkey: WalletPubkeyHex): WarrenTunnelConfig {
        val daitaEnabled = localSettings.daitaEnabled.value
        val natPmpEnabled = localSettings.natPmpEnabled.value
        val multiHopEnabled = localSettings.multiHopEnabled.value
        val obfuscationM40 = localSettings.obfuscationM40.value

        return WarrenTunnelConfig(
            exitPubkeyHex = DEFAULT_EXIT_PUBKEY_HEX,
            exitEndpoint = DEFAULT_EXIT_ENDPOINT,
            walletPubkeyHex = walletPubkey.value,
            entryHop = if (multiHopEnabled) {
                // TODO (D.4 step 8): swap for relay-selector picked entry
                //   relay once the Warren relay list is exposed via JNI.
                WarrenTunnelConfig.EntryHop(
                    relayPubkeyHex = DEFAULT_ENTRY_RELAY_PUBKEY_HEX,
                    relayEndpoint = DEFAULT_ENTRY_RELAY_ENDPOINT,
                )
            } else null,
            daita = if (daitaEnabled) {
                // Single Tamaraw machine for now; picker UI lands D.6.
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
        // warren-exit-1 (Hetzner fsn1-dc14, persistent exit_id from
        // Session E memory `warren_session_e_delivered.md`).
        // TODO (D.4 step 8): replace with relay-selector output once the
        //   Warren relay list is exposed via `WarrenJni.listRelays()`.
        const val DEFAULT_EXIT_PUBKEY_HEX = "2921abad869e94064b56cf48c8da3631"
        const val DEFAULT_EXIT_ENDPOINT = "warren-exit-1.warren.brown:443"

        // Placeholder entry relay for multi-hop until the relay selector
        // is wired; will be replaced by a picker-chosen relay.
        const val DEFAULT_ENTRY_RELAY_PUBKEY_HEX = "0000000000000000000000000000000000000000000000000000000000000000"
        const val DEFAULT_ENTRY_RELAY_ENDPOINT = "warren-relay-1.warren.brown:443"

        // Tamaraw is the single padding machine warren-core ships today;
        // see warren-core `daita::TAMARAW_PADDING_MACHINE`.
        const val DEFAULT_DAITA_MACHINE = "tamaraw"
    }
}
