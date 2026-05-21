package com.warrenbrowse.vpn.app.connect

import com.warrenbrowse.vpn.app.service.WarrenTunnelConfig
import com.warrenbrowse.vpn.lib.model.wallet.WalletPubkeyHex

/**
 * Composes a [WarrenTunnelConfig] from in-memory state.
 *
 * D.4 step 7 cut: the only inputs the wiring needs *right now* are the
 * wallet pubkey (so the exit can authorise the session) and a stable
 * exit identity. Multi-hop entry resolution, DAITA spec selection,
 * NAT-PMP toggling, obfuscation and bypass-CIDR plumbing are kept
 * behind opt-in setters so the call site stays tiny while the
 * accessors for those settings (D.4 step 8+) come online.
 *
 * For the very first end-to-end smoke we point at warren-exit-1 prod
 * (the same exit the bench has been hitting in Session F-M). Once the
 * relay selector lands on Warren mobile, this builder is replaced by a
 * RelaySelector-driven path.
 */
class WarrenTunnelConfigBuilder {

    fun build(walletPubkey: WalletPubkeyHex): WarrenTunnelConfig =
        WarrenTunnelConfig(
            exitPubkeyHex = DEFAULT_EXIT_PUBKEY_HEX,
            exitEndpoint = DEFAULT_EXIT_ENDPOINT,
            walletPubkeyHex = walletPubkey.value,
            entryHop = null,
            daita = null,
            bypassCidrs = emptyList(),
            natPmpEnabled = false,
            obfuscationM40 = false,
        )

    private companion object {
        // warren-exit-1 (Hetzner fsn1-dc14, persistent exit_id from
        // Session E memory `warren_session_e_delivered.md`).
        // TODO (D.4 step 8): replace with relay-selector output once the
        //   Warren relay list is exposed via `WarrenJni.listRelays()`.
        const val DEFAULT_EXIT_PUBKEY_HEX = "2921abad869e94064b56cf48c8da3631"
        const val DEFAULT_EXIT_ENDPOINT = "warren-exit-1.warren.brown:443"
    }
}
