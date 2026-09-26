package com.warrenbrowse.vpn.lib.model

import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Test

/**
 * The snapshot is parsed tolerantly: the server may add fields and enum values at any time
 * without bumping the version, and a client that shipped before them must keep rendering.
 */
class WarrenNetworkStatsParserTest {

    private val fixture: String =
        requireNotNull(javaClass.getResource("/network-stats-v1.json")) {
                "the frozen contract fixture is a test resource"
            }
            .readText()

    private fun fixtureObject(): JsonObject = Json.parseToJsonElement(fixture).jsonObject

    private fun JsonObject.with(key: String, value: JsonElement?): JsonObject =
        JsonObject(if (value == null) this - key else this + (key to value))

    private fun JsonObject.withExit(index: Int, key: String, value: JsonElement?): JsonObject {
        val exits = getValue("exits").jsonArray.toMutableList()
        exits[index] = exits[index].jsonObject.with(key, value)
        return with("exits", JsonArray(exits))
    }

    @Test
    fun `reads every field of the frozen contract fixture`() {
        val stats = WarrenNetworkStatsParser.parse(fixture)!!

        assertEquals("beta", stats.environment)
        assertEquals(1_790_000_000L, stats.generatedAt)
        assertEquals(60, stats.windowSecs)
        assertEquals(5, stats.exitUsersRounding)
        assertEquals(20, stats.exitLiveThreshold)
        assertEquals(NetworkUsers(1234, 987, 57), stats.users)
        assertEquals(
            FleetStats(
                exitsOnline = 2,
                exitsTotal = 3,
                downloadBps = 400_000_000,
                uploadBps = 50_000_000,
                capacityBps = 2_000_000_000,
                loadPercent = 23,
                loadLevel = LoadLevel.LOW,
                transferred24hBytes = 9_000_000_000_000,
                peakConnected24h = 80,
                peakThroughput24hBps = 900_000_000,
            ),
            stats.fleet,
        )
        assertEquals(
            ExitStats(
                exitId = "abababababababababababababababab",
                name = "fr-par-h1b",
                country = "FR",
                city = "Paris",
                online = true,
                live = true,
                connected = 40,
                downloadBps = 300_000_000,
                uploadBps = 30_000_000,
                capacityBps = 1_000_000_000,
                loadPercent = 37,
                loadLevel = LoadLevel.LOW,
                loadDriver = LoadDriver.BANDWIDTH,
                cpuPercent = 21,
                history = listOf(ExitHistoryPoint(1_789_999_940, 40, 310_000_000, 35)),
            ),
            stats.exits[0],
        )
        assertEquals(listOf(FleetHistoryPoint(1_789_999_100, 55, 420_000_000)), stats.history)
    }

    @Test
    fun `a quiet exit keeps its absent figures null rather than zero`() {
        val quiet = WarrenNetworkStatsParser.parse(fixture)!!.exits[1]

        assertEquals(false, quiet.live)
        assertEquals(LoadLevel.MODERATE, quiet.loadLevel)
        assertNull(quiet.loadPercent)
        assertNull(quiet.loadDriver)
        assertNull(quiet.cpuPercent)
        assertNull(quiet.capacityBps)
        assertNull(quiet.name)
        assertEquals(ExitDisplayMode.BAND, quiet.displayMode)
    }

    @Test
    fun `ignores fields it does not know, at every level`() {
        val doc =
            fixtureObject()
                .with("future_block", JsonObject(mapOf("x" to JsonPrimitive(1))))
                .withExit(0, "uptime_secs", JsonPrimitive(172_800))
                .withExit(0, "future_field", JsonArray(listOf(JsonPrimitive("y"))))

        val stats = WarrenNetworkStatsParser.parse(doc.toString())

        assertEquals(WarrenNetworkStatsParser.parse(fixture), stats)
    }

    @Test
    fun `an unknown band or driver reads as unknown, never as a known one`() {
        val doc =
            fixtureObject()
                .withExit(0, "load_level", JsonPrimitive("overloaded"))
                .withExit(0, "load_driver", JsonPrimitive("memory"))

        val exit = WarrenNetworkStatsParser.parse(doc.toString())!!.exits[0]

        assertEquals(LoadLevel.UNKNOWN, exit.loadLevel)
        assertEquals(LoadDriver.UNKNOWN, exit.loadDriver)
    }

    @Test
    fun `a fleet without a band reads as unknown`() {
        val fleet = fixtureObject().getValue("fleet").jsonObject.with("load_level", null)

        val stats = WarrenNetworkStatsParser.parse(fixtureObject().with("fleet", fleet).toString())

        assertEquals(LoadLevel.UNKNOWN, stats!!.fleet.loadLevel)
    }

    @Test
    fun `one unreadable exit costs that exit, not the snapshot`() {
        val doc = fixtureObject().withExit(1, "country", null)

        val stats = WarrenNetworkStatsParser.parse(doc.toString())!!

        assertEquals(listOf("abababababababababababababababab"), stats.exits.map { it.exitId })
    }

    @Test
    fun `an exit id that is not 32 hex chars drops the exit, and a valid one is lowercased`() {
        val doc =
            fixtureObject()
                .withExit(0, "exit_id", JsonPrimitive("ABABABABABABABABABABABABABABABAB"))
                .withExit(1, "exit_id", JsonPrimitive("not-hex"))

        val stats = WarrenNetworkStatsParser.parse(doc.toString())!!

        assertEquals(listOf("abababababababababababababababab"), stats.exits.map { it.exitId })
    }

    @Test
    fun `percentages above 100 are clamped and negative figures are absent`() {
        val doc =
            fixtureObject()
                .withExit(0, "load_percent", JsonPrimitive(140))
                .withExit(0, "cpu_percent", JsonPrimitive(-3))

        val exit = WarrenNetworkStatsParser.parse(doc.toString())!!.exits[0]

        assertEquals(100, exit.loadPercent)
        assertNull(exit.cpuPercent)
    }

    @Test
    fun `a missing required top-level figure makes the snapshot unreadable`() {
        assertNull(WarrenNetworkStatsParser.parse(fixtureObject().with("users", null).toString()))
        assertNull(
            WarrenNetworkStatsParser.parse(fixtureObject().with("generated_at", null).toString())
        )
    }

    @Test
    fun `a zero window is refused, it would poll in a loop`() {
        val doc = fixtureObject().with("window_secs", JsonPrimitive(0))

        assertNull(WarrenNetworkStatsParser.parse(doc.toString()))
    }

    @Test
    fun `a zero rounding step reads as one`() {
        val doc = fixtureObject().with("exit_users_rounding", JsonPrimitive(0))

        assertEquals(1, WarrenNetworkStatsParser.parse(doc.toString())!!.exitUsersRounding)
    }

    @Test
    fun `a document at another version is refused`() {
        val doc = fixtureObject().with("version", JsonPrimitive(2))

        assertNull(WarrenNetworkStatsParser.parse(doc.toString()))
    }

    @Test
    fun `text that is not a JSON object is refused`() {
        assertNull(WarrenNetworkStatsParser.parse("[]"))
        assertNull(WarrenNetworkStatsParser.parse("<html>"))
    }

    @Test
    fun `the ok envelope yields the snapshot`() {
        val fetch = WarrenNetworkStatsParser.parseEnvelope("""{"ok":true,"stats":$fixture}""")

        assertEquals(
            NetworkStatsFetch.Snapshot(WarrenNetworkStatsParser.parse(fixture)!!),
            fetch,
        )
    }

    @Test
    fun `a 404 and a schema this build cannot read are both unavailable`() {
        assertEquals(
            NetworkStatsFetch.Unavailable,
            WarrenNetworkStatsParser.parseEnvelope("""{"ok":false,"reason":"unavailable"}"""),
        )
        assertEquals(
            NetworkStatsFetch.Unavailable,
            WarrenNetworkStatsParser.parseEnvelope("""{"ok":false,"reason":"version"}"""),
        )
    }

    @Test
    fun `every other refusal is a transient failure`() {
        for (reason in listOf("transport", "status", "malformed", "too_large", "future")) {
            assertEquals(
                NetworkStatsFetch.Failed,
                WarrenNetworkStatsParser.parseEnvelope("""{"ok":false,"reason":"$reason"}"""),
                reason,
            )
        }
    }

    @Test
    fun `an ok envelope whose snapshot cannot be read is a failure`() {
        assertEquals(
            NetworkStatsFetch.Failed,
            WarrenNetworkStatsParser.parseEnvelope("""{"ok":true,"stats":{"version":1}}"""),
        )
    }

    @Test
    fun `an envelope that is not JSON is a failure, not a crash`() {
        assertEquals(NetworkStatsFetch.Failed, WarrenNetworkStatsParser.parseEnvelope("null"))
        assertEquals(NetworkStatsFetch.Failed, WarrenNetworkStatsParser.parseEnvelope("{"))
    }
}
