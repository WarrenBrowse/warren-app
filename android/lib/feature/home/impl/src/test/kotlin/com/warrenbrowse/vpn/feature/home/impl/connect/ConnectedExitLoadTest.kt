package com.warrenbrowse.vpn.feature.home.impl.connect

import com.warrenbrowse.vpn.lib.model.Endpoint
import com.warrenbrowse.vpn.lib.model.ExitStats
import com.warrenbrowse.vpn.lib.model.FleetStats
import com.warrenbrowse.vpn.lib.model.LoadLevel
import com.warrenbrowse.vpn.lib.model.NetworkUsers
import com.warrenbrowse.vpn.lib.model.TransportProtocol
import com.warrenbrowse.vpn.lib.model.WarrenNetworkStats
import com.warrenbrowse.vpn.lib.repository.WarrenRelaySummary
import java.net.InetSocketAddress
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Test

internal object NetworkStatsFixtures {
    const val EXIT_ID = "abababababababababababababababab"

    val exit =
        ExitStats(
            exitId = EXIT_ID,
            name = null,
            country = "FR",
            city = "Paris",
            online = true,
            live = false,
            connected = 0,
            downloadBps = 0,
            uploadBps = 0,
            capacityBps = null,
            loadPercent = null,
            loadLevel = LoadLevel.MODERATE,
            loadDriver = null,
            cpuPercent = null,
            history = emptyList(),
        )

    val stats =
        WarrenNetworkStats(
            environment = "beta",
            generatedAt = 0,
            windowSecs = 60,
            exitUsersRounding = 5,
            exitLiveThreshold = 20,
            users = NetworkUsers(0, 0, 0),
            fleet = FleetStats(0, 0, 0, 0, 0, 0, LoadLevel.LOW, 0, 0, 0),
            exits = listOf(exit),
            history = emptyList(),
        )

    val relay =
        WarrenRelaySummary(
            exitId = EXIT_ID,
            exitPubkeyHex = "",
            endpoint = "192.0.2.7:443",
            country = "FR",
            city = "Paris",
            active = true,
            weight = 1,
        )

    fun endpoint(host: String) = Endpoint(InetSocketAddress(host, 443), TransportProtocol.Udp)
}

class ConnectedExitLoadTest {
    private val fixtures = NetworkStatsFixtures

    @Test
    fun `the exit is found through the relay whose endpoint the tunnel runs on`() {
        val other = fixtures.exit.copy(exitId = "cd".repeat(16), city = "Lyon")
        val stats = fixtures.stats.copy(exits = listOf(other, fixtures.exit))

        val load = connectedExitLoad(fixtures.endpoint("192.0.2.7"), listOf(fixtures.relay), stats)

        assertEquals(ConnectedExitLoad(fixtures.exit, stats), load)
    }

    @Test
    fun `an endpoint no relay carries has no load`() {
        assertNull(
            connectedExitLoad(
                fixtures.endpoint("192.0.2.8"),
                listOf(fixtures.relay),
                fixtures.stats,
            )
        )
    }

    @Test
    fun `no snapshot, or an exit it does not list, has no load`() {
        assertNull(connectedExitLoad(fixtures.endpoint("192.0.2.7"), listOf(fixtures.relay), null))
        assertNull(
            connectedExitLoad(
                fixtures.endpoint("192.0.2.7"),
                listOf(fixtures.relay),
                fixtures.stats.copy(exits = emptyList()),
            )
        )
    }
}
