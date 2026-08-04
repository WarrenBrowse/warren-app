package com.warrenbrowse.vpn.app.service

import kotlinx.serialization.json.Json
import kotlinx.serialization.json.jsonObject
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

/**
 * Locks the Kotlin -> warren-jni wire contract for [WarrenTunnelConfig].
 *
 * The config is serialised to JSON in [WarrenQuinnAdapter], shipped through
 * the VpnService Intent, and deserialised by the Rust `parse_config`
 * (`warren-jni/src/tunnel.rs`) via serde. Neither side uses
 * `deny_unknown_fields`, so a drifted `@SerialName` would silently
 * deserialise to a default/None on the Rust side instead of erroring. These
 * tests pin the exact wire key names so such drift fails loudly here.
 *
 * The expected key sets below MUST mirror the serde field names of the Rust
 * `WarrenTunnelConfig` / `DnsConfig` structs.
 */
class WarrenTunnelConfigSerializationTest {

    private val fullConfig = WarrenTunnelConfig(
        exitPubkeyHex = "ab".repeat(32),
        exitEndpoint = "1.2.3.4:443",
        walletPubkeyHex = "wb7kgy8FF4rx4tamkksPfoymeeeZVXLrnSjbBxCun3XhP9DnB",
        entryHop = WarrenTunnelConfig.EntryHop(
            relayPubkeyHex = "cd".repeat(32),
            relayEndpoint = "5.6.7.8:443",
        ),
        // Non-default (default is true) so the key is emitted and pinned here;
        // kotlinx omits default-valued fields (encodeDefaults = false).
        multihopTwoHop = false,
        daita = WarrenTunnelConfig.DaitaSpec(
            paddingMachine = "tamaraw",
            normalizePackets = false,
        ),
        natPmpEnabled = true,
        natPmpProtocol = "tcp",
        natPmpExternalPort = 51820,
        natPmpLifetimeSecs = 21600,
        enableIpv6 = true,
        lockdownMode = true,
        dns = WarrenTunnelConfig.DnsConfig(
            state = WarrenTunnelConfig.DnsConfig.STATE_CUSTOM,
            customServers = listOf("9.9.9.9"),
            blockAds = true,
            blockTrackers = true,
            blockMalware = true,
            blockAdultContent = true,
            blockGambling = true,
            blockSocialMedia = true,
        ),
        allowLan = true,
        mtu = 1200,
    )

    @Test
    fun `top-level wire keys match the Rust serde contract`() {
        val json = fullConfig.toWireJson()
        val keys = Json.parseToJsonElement(json).jsonObject.keys

        val expected = setOf(
            "exit_pubkey_hex",
            "exit_endpoint",
            "wallet_pubkey_hex",
            "entry_hop",
            "multihop_two_hop",
            "daita",
            "nat_pmp_enabled",
            "nat_pmp_protocol",
            "nat_pmp_external_port",
            "nat_pmp_lifetime_secs",
            "enable_ipv6",
            "lockdown_mode",
            "dns",
            // Android-only knobs the Rust struct ignores (no deny_unknown_fields).
            "allow_lan",
            "mtu",
        )
        assertEquals(expected, keys)
    }

    @Test
    fun `nested entry_hop, daita and dns wire keys match the Rust serde contract`() {
        val json = fullConfig.toWireJson()
        val root = Json.parseToJsonElement(json).jsonObject

        assertEquals(
            setOf("relay_pubkey_hex", "relay_endpoint"),
            root.getValue("entry_hop").jsonObject.keys,
        )
        assertEquals(
            setOf("padding_machine", "normalize_packets"),
            root.getValue("daita").jsonObject.keys,
        )
        assertEquals(
            setOf(
                "state",
                "custom_servers",
                "block_ads",
                "block_trackers",
                "block_malware",
                "block_adult_content",
                "block_gambling",
                "block_social_media",
            ),
            root.getValue("dns").jsonObject.keys,
        )
    }

    @Test
    fun `config round-trips through JSON unchanged`() {
        val decoded = warrenTunnelConfigFromWireJson(fullConfig.toWireJson())
        assertEquals(fullConfig, decoded)
    }

    @Test
    fun `required exit fields serialise under their snake_case names`() {
        val json = fullConfig.toWireJson()
        assertTrue(json.contains("\"exit_pubkey_hex\":\"${"ab".repeat(32)}\""))
        assertTrue(json.contains("\"exit_endpoint\":\"1.2.3.4:443\""))
    }
}
