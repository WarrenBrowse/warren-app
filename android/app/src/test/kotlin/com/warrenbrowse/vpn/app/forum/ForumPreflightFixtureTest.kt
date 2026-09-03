package com.warrenbrowse.vpn.app.forum

import com.warrenbrowse.vpn.fixtures.ClientRulesFixtures
import com.warrenbrowse.vpn.fixtures.ClientRulesFixtures.cases
import com.warrenbrowse.vpn.fixtures.ClientRulesFixtures.string
import com.warrenbrowse.vpn.fixtures.ClientRulesFixtures.stringOrNull
import com.warrenbrowse.vpn.lib.repository.ForumPreflight
import com.warrenbrowse.vpn.lib.repository.WarrenConnectedInfo
import kotlinx.serialization.json.JsonObject
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

/**
 * The tunnel-state preflight replayed from `fixtures/client-rules/forum_preflight.json`, the file
 * the iOS reader replays on its side. Both clients hold the same verdict for the same tunnel, and a
 * divergence fails here rather than showing up as a forum request that hangs on one platform only.
 */
class ForumPreflightFixtureTest {

    private val fixture = ClientRulesFixtures.load("forum_preflight.json")

    @Test
    fun every_shared_tunnel_state_gets_the_fixtures_verdict() {
        val cases = fixture.cases("cases").filterNot(ClientRulesFixtures::skippedOnAndroid)
        assertTrue(cases.size >= 8, "only ${cases.size} preflight cases reached this reader")
        for (case in cases) {
            val name = case.string("name")
            val verdict = ForumPreflight.of(tunnel(case.string("tunnel")))
            assertEquals(expected(case), verdict, name)
        }
    }

    private fun expected(case: JsonObject): ForumPreflight {
        val expect = requireNotNull(case["expect"]) { "expect is missing in $case" } as JsonObject
        return when (val kind = expect.string("verdict")) {
            "proceed" -> ForumPreflight.Proceed
            "defer" -> ForumPreflight.Defer(requireNotNull(expect.stringOrNull("class")))
            else -> error("unknown verdict $kind")
        }
    }

    /** The fixture's platform-neutral state name as this client spells it. */
    private fun tunnel(name: String): WarrenConnectedInfo =
        when (name) {
            "disconnected" -> WarrenConnectedInfo.Disconnected
            "connected" ->
                WarrenConnectedInfo.Connected(
                    exitEndpointHost = "203.0.113.7:443",
                    entryEndpointHost = null,
                    multiHop = false,
                    daita = false,
                    assignedNatPmpPort = null,
                )
            "connecting" -> WarrenConnectedInfo.Connecting()
            "reconnecting" -> WarrenConnectedInfo.Reconnecting()
            "disconnecting" -> WarrenConnectedInfo.Disconnecting(reconnecting = false)
            "disconnecting_to_reconnect" -> WarrenConnectedInfo.Disconnecting(reconnecting = true)
            "blocking" -> WarrenConnectedInfo.Blocking("kill switch")
            "failed_released" -> WarrenConnectedInfo.Failed("exit refused")
            else -> error("unknown tunnel state $name")
        }
}
