package com.warrenbrowse.vpn.app.service

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

// JSON payload handed to `WarrenJni.connectTunnel`. Lives in this module
// rather than `lib/model` because the Rust side reads it via `serde_json`
// directly - no `kotlinx-serialization` <-> protobuf marshalling to worry
// about. Field names mirror `warren_tunnel::ClientConfig` so the Rust
// `serde::Deserialize` derive lines up 1:1.
//
// D.4 scaffold. The actual Rust deserializer lives in
// `warren-jni/src/lib.rs::Java_..._connectTunnel` (TODO marker today).
@Serializable
data class WarrenTunnelConfig(
    @SerialName("exit_pubkey_hex") val exitPubkeyHex: String,
    @SerialName("exit_endpoint") val exitEndpoint: String,
    @SerialName("wallet_pubkey_hex") val walletPubkeyHex: String,
    @SerialName("entry_hop") val entryHop: EntryHop? = null,
    @SerialName("daita") val daita: DaitaSpec? = null,
    @SerialName("bypass_cidrs") val bypassCidrs: List<String> = emptyList(),
    @SerialName("nat_pmp_enabled") val natPmpEnabled: Boolean = false,
    @SerialName("obfuscation_m40") val obfuscationM40: Boolean = false,
) {
    @Serializable
    data class EntryHop(
        @SerialName("relay_pubkey_hex") val relayPubkeyHex: String,
        @SerialName("relay_endpoint") val relayEndpoint: String,
    )

    @Serializable
    data class DaitaSpec(
        @SerialName("padding_machine") val paddingMachine: String,
        @SerialName("normalize_packets") val normalizePackets: Boolean = true,
    )
}
