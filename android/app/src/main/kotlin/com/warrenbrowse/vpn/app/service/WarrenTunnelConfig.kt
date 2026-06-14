package com.warrenbrowse.vpn.app.service

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json

// JSON payload handed to `WarrenJni.connectTunnel`. Lives in this module
// rather than `lib/model` because the Rust side reads it via `serde_json`
// directly - no `kotlinx-serialization` <-> protobuf marshalling to worry
// about. Field names mirror `warren_tunnel::ClientConfig` so the Rust
// `serde::Deserialize` derive lines up 1:1.
//
// The Rust deserializer lives in
// `warren-jni/src/lib.rs::Java_..._connectTunnel` (TODO marker today).
@Serializable
data class WarrenTunnelConfig(
    @SerialName("exit_pubkey_hex") val exitPubkeyHex: String,
    @SerialName("exit_endpoint") val exitEndpoint: String,
    @SerialName("wallet_pubkey_hex") val walletPubkeyHex: String,
    @SerialName("entry_hop") val entryHop: EntryHop? = null,
    @SerialName("daita") val daita: DaitaSpec? = null,
    @SerialName("nat_pmp_enabled") val natPmpEnabled: Boolean = false,
    // NAT-PMP / port-forwarding parameters. Honoured by the refresh loop in
    // warren-natpmp-client. `protocol` is "udp" or "tcp"; `externalPort` 0
    // means "let the gateway pick"; `lifetimeSecs` is the requested mapping
    // lifetime (the gateway may cap it).
    @SerialName("nat_pmp_protocol") val natPmpProtocol: String = "udp",
    @SerialName("nat_pmp_external_port") val natPmpExternalPort: Int = 0,
    @SerialName("nat_pmp_lifetime_secs") val natPmpLifetimeSecs: Int = 3600,
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
    // Local network sharing ("allow LAN"). When true, RFC1918 / link-local
    // ranges are excluded from the TUN routes so the device can reach LAN
    // hosts (printers, NAS, casting) directly while everything else stays
    // tunnelled. Enforced entirely Android-side in WarrenTunInterfacePlan.
    //
    // Serialized (not @Transient) because the config is JSON round-tripped
    // through the VpnService Intent before reaching the adapter/plan, so a
    // @Transient value would be lost in transit. The warren-jni Rust side
    // ignores this unknown field (its WarrenTunnelConfig has no
    // deny_unknown_fields), so no wire/Rust change is needed.
    @SerialName("allow_lan") val allowLan: Boolean = false,
    // TUN interface MTU. Lower it on networks that mangle large packets;
    // the default is the Warren QUIC floor. Android-side only (sets
    // VpnService.Builder.setMtu); the Rust side ignores this unknown field.
    @SerialName("mtu") val mtu: Int = 1280,
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

/**
 * Serialise to the JSON wire form handed to `WarrenJni.connectTunnel` and
 * read by the Rust `parse_config`. Defined here (in the main source set,
 * where the kotlinx-serialization compiler plugin is applied) so callers and
 * tests share one encode path; the generated serializer is not resolvable
 * from the unit-test source set.
 */
fun WarrenTunnelConfig.toWireJson(): String = Json.encodeToString(this)

/** Inverse of [toWireJson]; parses the wire form back into a config. */
fun warrenTunnelConfigFromWireJson(json: String): WarrenTunnelConfig =
    Json.decodeFromString(json)

