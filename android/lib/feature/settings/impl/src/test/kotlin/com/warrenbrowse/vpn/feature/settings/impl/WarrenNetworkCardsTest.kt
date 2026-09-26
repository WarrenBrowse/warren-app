package com.warrenbrowse.vpn.feature.settings.impl

import com.warrenbrowse.vpn.lib.model.ExitStats
import com.warrenbrowse.vpn.lib.model.FleetStats
import com.warrenbrowse.vpn.lib.model.LoadLevel
import com.warrenbrowse.vpn.lib.model.NetworkUsers
import com.warrenbrowse.vpn.lib.model.WarrenNetworkStats
import com.warrenbrowse.vpn.lib.repository.WarrenRelaySummary
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Test

class WarrenNetworkCardsTest {

    private fun exit(id: Char, country: String, city: String, online: Boolean = true) =
        ExitStats(
            exitId = id.toString().repeat(32),
            name = null,
            country = country,
            city = city,
            online = online,
            live = false,
            connected = 0,
            downloadBps = 0,
            uploadBps = 0,
            capacityBps = null,
            loadPercent = null,
            loadLevel = LoadLevel.LOW,
            loadDriver = null,
            cpuPercent = null,
            history = emptyList(),
        )

    private fun relay(id: Char, country: String, city: String) =
        WarrenRelaySummary(
            exitId = id.toString().repeat(32),
            exitPubkeyHex = "",
            endpoint = "192.0.2.1:443",
            country = country,
            city = city,
            active = true,
            weight = 1,
        )

    private fun stats(vararg exits: ExitStats) =
        WarrenNetworkStats(
            environment = "beta",
            generatedAt = 0,
            windowSecs = 60,
            exitUsersRounding = 5,
            exitLiveThreshold = 20,
            users = NetworkUsers(0, 0, 0),
            fleet = FleetStats(0, 0, 0, 0, 0, 0, LoadLevel.LOW, 0, 0, 0),
            exits = exits.toList(),
            history = emptyList(),
        )

    @Test
    fun `the place comes from the signed relay list, not the snapshot`() {
        val cards =
            networkExitCards(stats(exit('a', "XX", "Nowhere")), listOf(relay('a', "FR", "Paris")))

        assertEquals("FR" to "Paris", cards.single().country to cards.single().city)
    }

    @Test
    fun `an exit the relay list does not carry keeps the snapshot's place`() {
        val cards = networkExitCards(stats(exit('b', "RO", "Bucharest")), emptyList())

        assertEquals("RO" to "Bucharest", cards.single().country to cards.single().city)
    }

    @Test
    fun `serving exits come first, then by country and city`() {
        val cards =
            networkExitCards(
                stats(
                    exit('a', "SE", "Stockholm", online = false),
                    exit('b', "FR", "Paris"),
                    exit('c', "DE", "Berlin"),
                    exit('d', "FR", "Lyon"),
                ),
                emptyList(),
            )

        assertEquals(listOf("Berlin", "Lyon", "Paris", "Stockholm"), cards.map { it.city })
    }
}
