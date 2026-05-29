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
    // Privacy-leak controls (P0). All default to the leak-safe value so an
    // older builder that omits them stays protected.
    //
    // `enableIpv6 = false` mirrors the desktop default: IPv6 is captured by
    // the TUN and blackholed rather than routed to the underlying network.
    @SerialName("enable_ipv6") val enableIpv6: Boolean = false,
    // App-level kill switch. When true the adapter keeps a blocking TUN in
    // place if the tunnel drops instead of returning traffic to the
    // physical network. Mirrors the desktop `lockdownMode`.
    @SerialName("lockdown_mode") val lockdownMode: Boolean = false,
    // DNS routing. `null` => use the exit's in-tunnel forwarder (10.66.0.1).
    // Set to route DNS through the tunnel (anti-leak) and/or push custom
    // resolvers and exit-side content-blocking flags.
    @SerialName("dns") val dns: DnsConfig? = null,
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

    /**
     * DNS options handed to the tunnel. Mirrors the desktop `IDnsOptions`
     * split between a `default` mode (exit forwarder + optional content
     * blocking) and a `custom` mode (explicit resolver addresses).
     *
     * Content-blocking flags are honoured exit-side by the Warren DNS
     * forwarder; the client only routes DNS into the tunnel so the
     * queries never leak to the LAN resolver.
     */
    @Serializable
    data class DnsConfig(
        // "default" | "custom"
        @SerialName("state") val state: String = STATE_DEFAULT,
        @SerialName("custom_servers") val customServers: List<String> = emptyList(),
        @SerialName("block_ads") val blockAds: Boolean = false,
        @SerialName("block_trackers") val blockTrackers: Boolean = false,
        @SerialName("block_malware") val blockMalware: Boolean = false,
        @SerialName("block_adult_content") val blockAdultContent: Boolean = false,
        @SerialName("block_gambling") val blockGambling: Boolean = false,
        @SerialName("block_social_media") val blockSocialMedia: Boolean = false,
    ) {
        companion object {
            const val STATE_DEFAULT = "default"
            const val STATE_CUSTOM = "custom"
        }
    }
}
