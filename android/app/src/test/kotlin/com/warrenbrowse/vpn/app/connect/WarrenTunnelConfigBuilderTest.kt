package com.warrenbrowse.vpn.app.connect

import com.warrenbrowse.vpn.app.service.WarrenTunnelConfig
import com.warrenbrowse.vpn.lib.model.wallet.WalletAddress
import com.warrenbrowse.vpn.lib.repository.ExitPin
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository
import com.warrenbrowse.vpn.lib.repository.WarrenProductFlags
import io.mockk.every
import io.mockk.mockk
import java.util.concurrent.atomic.AtomicInteger
import kotlin.time.Duration.Companion.hours
import kotlin.time.TestTimeSource
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.runBlocking
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertNotNull
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

class WarrenTunnelConfigBuilderTest {

    private val pubkey = WalletAddress("wb7kgy8FF4rx4tamkksPfoymeeeZVXLrnSjbBxCun3XhP9DnB")

    private val sampleRelay = RelayInfo(
        exitId = "2921abad869e94064b56cf48c8da3631",
        exitPubkeyHex = "2921abad869e94064b56cf48c8da3631",
        endpoint = "warren-exit-1.warren.brown:443",
        country = "DE",
        city = "Falkenstein",
        active = true,
        weight = 100,
    )

    private fun mockRepo(
        daita: Boolean = false,
        natPmp: Boolean = false,
        // Mirrors the production default (multi-hop ON): single-hop is opt-in.
        multiHop: Boolean = true,
        maxRateBps: Long = 0L,
        selectedExitId: String? = null,
        exitPin: ExitPin? = null,
        entryCountry: String? = null,
        exitCountry: String? = null,
        natPmpProtocol: String = "udp",
        natPmpExternalPort: Int = 0,
        natPmpLifetimeSecs: Int = 3600,
        ipv6: Boolean = false,
        lockdown: Boolean = false,
        allowLan: Boolean = false,
        dnsState: String = WarrenLocalSettingsRepository.DNS_STATE_DEFAULT,
        customDns: List<String> = emptyList(),
        blockAds: Boolean = false,
        blockTrackers: Boolean = false,
        blockMalware: Boolean = false,
        blockAdult: Boolean = false,
        blockGambling: Boolean = false,
        blockSocial: Boolean = false,
    ): WarrenLocalSettingsRepository {
        val repo: WarrenLocalSettingsRepository = mockk()
        every { repo.daitaEnabled } returns MutableStateFlow(daita)
        every { repo.natPmpEnabled } returns MutableStateFlow(natPmp)
        every { repo.natPmpProtocol } returns MutableStateFlow(natPmpProtocol)
        every { repo.natPmpExternalPort } returns MutableStateFlow(natPmpExternalPort)
        every { repo.natPmpLifetimeSecs } returns MutableStateFlow(natPmpLifetimeSecs)
        every { repo.multiHopEnabled } returns MutableStateFlow(multiHop)
        every { repo.selectedExitId } returns MutableStateFlow(selectedExitId)
        every { repo.exitPin } returns MutableStateFlow(
            exitPin
                ?: selectedExitId?.let { ExitPin.Exit(it) }
                ?: ExitPin.Automatic
        )
        every { repo.entryCountry } returns MutableStateFlow(entryCountry)
        every { repo.exitCountry } returns MutableStateFlow(exitCountry)
        every { repo.ipv6Enabled } returns MutableStateFlow(ipv6)
        every { repo.lockdownMode } returns MutableStateFlow(lockdown)
        every { repo.allowLan } returns MutableStateFlow(allowLan)
        every { repo.dnsState } returns MutableStateFlow(dnsState)
        every { repo.customDnsServers } returns MutableStateFlow(customDns)
        every { repo.blockAds } returns MutableStateFlow(blockAds)
        every { repo.blockTrackers } returns MutableStateFlow(blockTrackers)
        every { repo.blockMalware } returns MutableStateFlow(blockMalware)
        every { repo.blockAdultContent } returns MutableStateFlow(blockAdult)
        every { repo.blockGambling } returns MutableStateFlow(blockGambling)
        every { repo.blockSocialMedia } returns MutableStateFlow(blockSocial)
        every { repo.maxRateBps } returns MutableStateFlow(maxRateBps)
        return repo
    }

    private fun mockCatalog(relays: List<RelayInfo> = listOf(sampleRelay)): RelayCatalog {
        val catalog: RelayCatalog = mockk()
        every { catalog.relaysForDial() } returns relays.map { it.toSummary() }
        every { catalog.list() } returns relays.map { it.toSummary() }
        return catalog
    }

    // Construct the builder with a stub multi-hop directory fetch so the unit
    // tests never touch the native WarrenJni (which would fail to load its lib
    // in the JVM). The stub is non-empty so build() does not short-circuit.
    private fun cfgBuilder(
        repo: WarrenLocalSettingsRepository,
        catalog: RelayCatalog,
        productFlags: WarrenProductFlags = WarrenProductFlags(isBeta = false),
    ) = WarrenTunnelConfigBuilder(repo, productFlags, catalog) { stubDirectory }

    private val stubDirectory = "stub-multihop-directory"

    @Test
    fun `default config requests multi-hop (empty entry hop) and no daita`() {
        val builder = cfgBuilder(mockRepo(), mockCatalog())
        val config = builder.build(pubkey)!!

        // Default topology is 2-hop: an empty entry_hop flips warren-jni to
        // auto-select a DISTINCT entry relay, and multihopTwoHop stays true.
        assertNotNull(config.entryHop)
        assertNull(config.entryHop?.relayPubkeyHex)
        assertTrue(config.multihopTwoHop)
        assertNull(config.daita)
        assertFalse(config.natPmpEnabled)
        assertEquals(pubkey.value, config.walletPubkeyHex)
        assertEquals(sampleRelay.exitPubkeyHex, config.exitPubkeyHex)
    }

    @Test
    fun `multi-hop off requests a single-hop circuit but still rides the multi-hop wire`() {
        val builder = cfgBuilder(mockRepo(multiHop = false), mockCatalog())
        val config = builder.build(pubkey)!!

        // Single-hop: warren-jni collapses the circuit onto the exit node.
        assertFalse(config.multihopTwoHop)
        // The tunnel still rides the multi-hop wire (the fleet speaks only
        // that), so entry_hop and the prefetched directory are still sent.
        assertNotNull(config.entryHop)
        assertNull(config.entryHop?.relayPubkeyHex)
        assertEquals(stubDirectory, config.multihopDirectoryRaw)
    }

    @Test
    fun `daita on injects a Tamaraw spec`() {
        val builder = cfgBuilder(mockRepo(daita = true), mockCatalog())
        val config = builder.build(pubkey)!!

        assertNotNull(config.daita)
        assertEquals("tamaraw", config.daita?.paddingMachine)
        assertTrue(config.daita?.normalizePackets == true)
    }

    @Test
    fun `builder always injects an empty entry hop to trigger multi-hop`() {
        // The fleet runs --multihop-only, so warren-jni routes through
        // run_multi_hop_session: a present-but-empty entry_hop is what flips it
        // to multi-hop (auto entry selection). It must never carry a pinned
        // entry pubkey from the builder (auto-select stays the contract).
        val builder = cfgBuilder(mockRepo(multiHop = true), mockCatalog())
        val config = builder.build(pubkey)!!

        assertNotNull(config.entryHop)
        assertNull(config.entryHop?.relayPubkeyHex)
        assertNull(config.entryHop?.relayEndpoint)
    }

    @Test
    fun `nat pmp toggle flows through`() {
        val builder = cfgBuilder(mockRepo(natPmp = true), mockCatalog())
        val config = builder.build(pubkey)!!
        assertTrue(config.natPmpEnabled)
    }

    @Test
    fun `empty catalogue yields null config`() {
        val builder = cfgBuilder(mockRepo(), mockCatalog(emptyList()))
        assertNull(builder.build(pubkey))
    }

    @Test
    fun `selectedExitId picks the matching relay when present`() {
        val otherRelay = sampleRelay.copy(
            exitId = "ffffffffffffffffffffffffffffffff",
            exitPubkeyHex = "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
            endpoint = "warren-exit-2.warren.brown:443",
        )
        val builder = cfgBuilder(
            mockRepo(selectedExitId = otherRelay.exitId),
            mockCatalog(listOf(sampleRelay, otherRelay)),
        )
        val config = builder.build(pubkey)!!
        assertEquals(otherRelay.exitPubkeyHex, config.exitPubkeyHex)
        assertEquals(otherRelay.endpoint, config.exitEndpoint)
    }

    @Test
    fun `selectedExitId falls back when target is inactive`() {
        val inactive = sampleRelay.copy(exitId = "deadbeefdeadbeefdeadbeefdeadbeef", active = false)
        val builder = cfgBuilder(
            mockRepo(selectedExitId = inactive.exitId),
            mockCatalog(listOf(sampleRelay, inactive)),
        )
        // Inactive selection falls back to the first active relay (sample).
        val config = builder.build(pubkey)!!
        assertEquals(sampleRelay.exitPubkeyHex, config.exitPubkeyHex)
    }

    @Test
    fun `a country pin resolves to an active exit in that country`() {
        val fr = sampleRelay.copy(
            exitId = "ffffffffffffffffffffffffffffffff",
            exitPubkeyHex = "f".repeat(64),
            endpoint = "warren-exit-fr.warren.brown:443",
            country = "FR",
            city = "Paris",
        )
        val builder = cfgBuilder(
            mockRepo(exitPin = ExitPin.Country("FR")),
            mockCatalog(listOf(sampleRelay, fr)),
        )
        val config = builder.build(pubkey)!!
        assertEquals(fr.exitPubkeyHex, config.exitPubkeyHex)
    }

    @Test
    fun `a city pin resolves to an active exit in that city`() {
        val berlin = sampleRelay.copy(
            exitId = "ffffffffffffffffffffffffffffffff",
            exitPubkeyHex = "f".repeat(64),
            endpoint = "warren-exit-ber.warren.brown:443",
            country = "DE",
            city = "Berlin",
        )
        val builder = cfgBuilder(
            mockRepo(exitPin = ExitPin.City("DE", "Berlin")),
            mockCatalog(listOf(sampleRelay, berlin)),
        )
        val config = builder.build(pubkey)!!
        assertEquals(berlin.exitPubkeyHex, config.exitPubkeyHex)
    }

    @Test
    fun `a country pin with nothing active falls back to the first active relay`() {
        val downFr = sampleRelay.copy(
            exitId = "ffffffffffffffffffffffffffffffff",
            exitPubkeyHex = "f".repeat(64),
            country = "FR",
            active = false,
        )
        val builder = cfgBuilder(
            mockRepo(exitPin = ExitPin.Country("FR")),
            mockCatalog(listOf(sampleRelay, downFr)),
        )
        val config = builder.build(pubkey)!!
        assertEquals(sampleRelay.exitPubkeyHex, config.exitPubkeyHex)
    }

    @Test
    fun `exit country selects a matching relay`() {
        val fr = sampleRelay.copy(
            exitId = "ffffffffffffffffffffffffffffffff",
            exitPubkeyHex = "f".repeat(64),
            endpoint = "warren-exit-fr.warren.brown:443",
            country = "FR",
        )
        val config = cfgBuilder(
            mockRepo(exitCountry = "FR"),
            mockCatalog(listOf(sampleRelay, fr)),
        ).build(pubkey)!!
        assertEquals(fr.exitPubkeyHex, config.exitPubkeyHex)
    }

    @Test
    fun `exit country falls back to first active when no relay matches`() {
        val config = cfgBuilder(
            mockRepo(exitCountry = "JP"),
            mockCatalog(listOf(sampleRelay)),
        ).build(pubkey)!!
        assertEquals(sampleRelay.exitPubkeyHex, config.exitPubkeyHex)
    }

    @Test
    fun `entry country does not pin the entry hop (warren-jni auto-selects)`() {
        val fr = sampleRelay.copy(
            exitId = "ffffffffffffffffffffffffffffffff",
            exitPubkeyHex = "f".repeat(64),
            endpoint = "warren-exit-fr.warren.brown:443",
            country = "FR",
        )
        val config = cfgBuilder(
            mockRepo(multiHop = true, entryCountry = "FR"),
            mockCatalog(listOf(sampleRelay, fr)),
        ).build(pubkey)!!
        // Exit defaults to first active (DE sample); the entry hop is present
        // but unpinned (warren-jni picks a distinct entry from the directory).
        assertEquals(sampleRelay.exitPubkeyHex, config.exitPubkeyHex)
        assertNotNull(config.entryHop)
        assertNull(config.entryHop?.relayPubkeyHex)
    }

    @Test
    fun `nat-pmp parameters flow through to the config`() {
        val config = cfgBuilder(
            mockRepo(natPmp = true, natPmpProtocol = "tcp", natPmpExternalPort = 51820, natPmpLifetimeSecs = 21600),
            mockCatalog(),
        ).build(pubkey)!!
        assertTrue(config.natPmpEnabled)
        assertEquals("tcp", config.natPmpProtocol)
        assertEquals(51820, config.natPmpExternalPort)
        assertEquals(21600, config.natPmpLifetimeSecs)
    }

    @Test
    fun `nat-pmp defaults are udp auto-port one-hour`() {
        val config = cfgBuilder(mockRepo(), mockCatalog()).build(pubkey)!!
        assertEquals("udp", config.natPmpProtocol)
        assertEquals(0, config.natPmpExternalPort)
        assertEquals(3600, config.natPmpLifetimeSecs)
    }

    @Test
    fun `ipv6 and lockdown default to the leak-safe values`() {
        val config = cfgBuilder(mockRepo(), mockCatalog()).build(pubkey)!!
        assertFalse(config.enableIpv6)
        assertFalse(config.lockdownMode)
    }

    @Test
    fun `ipv6 and lockdown toggles flow through`() {
        val config = cfgBuilder(
            mockRepo(ipv6 = true, lockdown = true),
            mockCatalog(),
        ).build(pubkey)!!
        assertTrue(config.enableIpv6)
        assertTrue(config.lockdownMode)
    }

    @Test
    fun `allow lan defaults off and flows through when enabled`() {
        assertFalse(cfgBuilder(mockRepo(), mockCatalog()).build(pubkey)!!.allowLan)
        val config = cfgBuilder(
            mockRepo(allowLan = true),
            mockCatalog(),
        ).build(pubkey)!!
        assertTrue(config.allowLan)
    }

    @Test
    fun `dns is null in default mode with no content blocking`() {
        val config = cfgBuilder(mockRepo(), mockCatalog()).build(pubkey)!!
        assertNull(config.dns)
    }

    @Test
    fun `custom dns servers flow into the config`() {
        val config = cfgBuilder(
            mockRepo(
                dnsState = WarrenLocalSettingsRepository.DNS_STATE_CUSTOM,
                customDns = listOf("9.9.9.9", "149.112.112.112"),
            ),
            mockCatalog(),
        ).build(pubkey)!!
        assertNotNull(config.dns)
        assertEquals("custom", config.dns?.state)
        assertEquals(listOf("9.9.9.9", "149.112.112.112"), config.dns?.customServers)
    }

    @Test
    fun `content blocking flags produce a default-mode dns config`() {
        val config = cfgBuilder(
            mockRepo(blockAds = true, blockMalware = true),
            mockCatalog(),
        ).build(pubkey)!!
        assertNotNull(config.dns)
        assertEquals("default", config.dns?.state)
        assertTrue(config.dns?.blockAds == true)
        assertTrue(config.dns?.blockMalware == true)
        assertFalse(config.dns?.blockTrackers == true)
        assertTrue(config.dns?.customServers?.isEmpty() == true)
    }

    @Test
    fun `all flags on produces a fully-populated config`() {
        val builder = cfgBuilder(
            mockRepo(daita = true, natPmp = true, multiHop = true),
            mockCatalog(),
        )
        val config = builder.build(pubkey)!!

        assertNotNull(config.entryHop)
        assertNotNull(config.daita)
        assertTrue(config.natPmpEnabled)
        assertEquals(pubkey.value, config.walletPubkeyHex)
    }

    @Test
    fun `user bandwidth cap flows into the config in bits per second`() {
        val builder = cfgBuilder(mockRepo(maxRateBps = 20_000_000L), mockCatalog())
        val config = builder.build(pubkey)!!
        assertEquals(20_000_000L, config.maxRateBps)
    }

    @Test
    fun `beta builds never send the user bandwidth cap`() {
        // The beta cap is network-imposed and server-enforced: whatever
        // value is persisted locally must not reach the tunnel config.
        val builder = cfgBuilder(
            mockRepo(maxRateBps = 20_000_000L),
            mockCatalog(),
            productFlags = WarrenProductFlags(isBeta = true),
        )
        val config = builder.build(pubkey)!!
        assertEquals(0L, config.maxRateBps)
    }

    // Failover: the drop retry moves the dropped session to another exit the
    // pin admits, resolved from the catalogue snapshot already in memory, and
    // redials the same exit when nothing else fits the pin.

    private val secondGermanRelay = sampleRelay.copy(
        exitId = "11111111111111111111111111111111",
        exitPubkeyHex = "1111111111111111111111111111111111111111111111111111111111111111",
        endpoint = "warren-exit-de-2.warren.brown:443",
        city = "Berlin",
        weight = 50,
    )

    private val frenchRelay = sampleRelay.copy(
        exitId = "22222222222222222222222222222222",
        exitPubkeyHex = "2222222222222222222222222222222222222222222222222222222222222222",
        endpoint = "warren-exit-fr.warren.brown:443",
        country = "FR",
        city = "Paris",
        weight = 80,
    )

    /** The session that dropped, as the adapter dialled it. */
    private val previous =
        WarrenTunnelConfig(
            exitPubkeyHex = sampleRelay.exitPubkeyHex,
            exitEndpoint = sampleRelay.endpoint,
            exitId = sampleRelay.exitId,
            walletPubkeyHex = pubkey.value,
            entryHop = WarrenTunnelConfig.EntryHop(),
            multihopDirectoryRaw = "the-directory-the-session-was-dialled-with",
            daita = WarrenTunnelConfig.DaitaSpec(paddingMachine = "tamaraw", normalizePackets = true),
        )

    @Test
    fun `a failover moves the session to another exit of the same country first`() {
        val builder =
            cfgBuilder(mockRepo(), mockCatalog(listOf(sampleRelay, secondGermanRelay, frenchRelay)))
        val config = builder.buildFailover(previous)!!
        assertEquals(secondGermanRelay.exitPubkeyHex, config.exitPubkeyHex)
        assertEquals(secondGermanRelay.endpoint, config.exitEndpoint)
        assertEquals(secondGermanRelay.exitId, config.exitId)
    }

    @Test
    fun `a failover leaves the country when it has no other exit`() {
        val builder = cfgBuilder(mockRepo(), mockCatalog(listOf(sampleRelay, frenchRelay)))
        val config = builder.buildFailover(previous)!!
        assertEquals(frenchRelay.exitPubkeyHex, config.exitPubkeyHex)
    }

    @Test
    fun `no failover when the pin allows only the exit that dropped`() {
        val builder =
            cfgBuilder(
                mockRepo(selectedExitId = sampleRelay.exitId),
                mockCatalog(listOf(sampleRelay, frenchRelay)),
            )
        assertNull(builder.buildFailover(previous))
    }

    @Test
    fun `a failover from an exit the catalogue no longer knows takes the normal pick`() {
        val builder = cfgBuilder(mockRepo(), mockCatalog(listOf(sampleRelay, frenchRelay)))
        val config = builder.buildFailover(previous.copy(exitPubkeyHex = "ff".repeat(32)))!!
        assertEquals(sampleRelay.exitPubkeyHex, config.exitPubkeyHex)
    }

    @Test
    fun `a failover keeps everything else the session was dialled with`() {
        // The directory travels with the session: run_multi_hop_session
        // verifies its signature and expiry at dial, so a stale one fails the
        // dial the way the same-config redial does, and a refetch would only
        // stall the retry behind the blackhole.
        val builder = cfgBuilder(mockRepo(), mockCatalog(listOf(sampleRelay, frenchRelay)))
        val config = builder.buildFailover(previous)!!
        assertEquals(previous.multihopDirectoryRaw, config.multihopDirectoryRaw)
        assertEquals(previous.daita, config.daita)
        assertEquals(previous.walletPubkeyHex, config.walletPubkeyHex)
    }

    @Test
    fun `a failover is resolved from the snapshot in memory and fetches nothing`() {
        // The drop retry runs behind the kill-switch blackhole, where no
        // unprotected socket leaves the device: a fetch there can only wait
        // out the transport's timeout before the redial starts. So the
        // failover reads the snapshot however old it is, the way the desktop
        // assemble_failover_for_attempt picks from the daemon's own list.
        val clock = TestTimeSource()
        val relayFetches = AtomicInteger()
        val catalog =
            RelayCatalog(timeSource = clock) {
                relayFetches.incrementAndGet()
                relaysJson(listOf(sampleRelay, frenchRelay))
            }
        runBlocking { catalog.refresh() }
        clock += 2.hours
        val directoryFetches = AtomicInteger()
        val builder =
            WarrenTunnelConfigBuilder(mockRepo(), WarrenProductFlags(isBeta = false), catalog) {
                directoryFetches.incrementAndGet()
                stubDirectory
            }

        val config = builder.buildFailover(previous)

        assertEquals(frenchRelay.exitPubkeyHex, config?.exitPubkeyHex)
        assertEquals(1, relayFetches.get(), "the stale snapshot is used, never refetched")
        assertEquals(0, directoryFetches.get(), "the directory is the session's own")
    }

    private fun relaysJson(relays: List<RelayInfo>): String =
        relays.joinToString(prefix = "[", postfix = "]") {
            """{"exit_id":"${it.exitId}","exit_pubkey_hex":"${it.exitPubkeyHex}",""" +
                """"endpoint":"${it.endpoint}","country":"${it.country}","city":"${it.city}",""" +
                """"active":${it.active},"weight":${it.weight}}"""
        }
}
