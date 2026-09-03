package com.warrenbrowse.vpn.app.connect

import androidx.test.platform.app.InstrumentationRegistry
import com.warrenbrowse.vpn.lib.repository.ExitPin
import com.warrenbrowse.vpn.lib.repository.WarrenRelaySummary
import kotlin.test.assertEquals
import kotlin.test.assertNull
import kotlin.test.assertTrue
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.longOrNull
import org.junit.jupiter.api.Test

/**
 * The shared `exit_pick.json` vector (warren-contract, `warren-discovery`),
 * replayed through the real `libwarren_jni.so` on the device: every candidate
 * becomes an active row of one country, the pin is that country, and the relay
 * the resolver hands back must be the vector's index. The Rust readers replay
 * the same file against `pick_exit` and through the daemon's own selection; this
 * is the Android end of the same contract. The file rides the test APK as an
 * asset (see the `androidTest` source set in `build.gradle.kts`).
 */
class ExitPinJniTest {
    private val resolver = JniExitPinResolver()

    private fun fixture() =
        InstrumentationRegistry.getInstrumentation()
            .context
            .assets
            .open("exit_pick.json")
            .bufferedReader()
            .use { Json.parseToJsonElement(it.readText()).jsonObject }

    private fun row(index: Int, exitId: String, weight: Long) =
        WarrenRelaySummary(
            exitId = exitId,
            exitPubkeyHex = index.toString(16).padStart(64, '0'),
            endpoint = "198.51.100.1:443",
            country = "XX",
            city = "City",
            active = true,
            weight = weight,
        )

    @Test
    fun exit_vectors_replay_through_the_native_resolver() {
        val cases = fixture()["exit"]!!.jsonArray.map { it.jsonObject }
        assertTrue(cases.size >= 8, "the exit section must keep its cases")
        for (case in cases) {
            val name = case["name"]!!.jsonPrimitive.content
            val rows =
                case["candidates"]!!.jsonArray.mapIndexed { i, c ->
                    val candidate = c.jsonObject
                    row(
                        i,
                        candidate["exit_id"]!!.jsonPrimitive.content,
                        // u64::MAX does not fit a Long: the JSON carries it as the
                        // unsigned literal, the same bytes Rust reads.
                        candidate["weight"]!!.jsonPrimitive.longOrNull ?: -1L,
                    )
                }
            if (rows.any { it.weight < 0 }) continue
            val picked = resolver.resolve(ExitPin.Country("xx"), rows)
            val expected = case["expected"]!!.jsonPrimitive.longOrNull?.toInt()?.let(rows::get)
            assertEquals(expected, picked, "exit vector `$name` diverged on the device")
        }
    }

    @Test
    fun the_saturating_weight_vector_replays_through_the_raw_contract() {
        // `weights_are_compared_never_summed` carries u64::MAX, which no Kotlin
        // Long holds, so it goes through the JSON boundary verbatim.
        val case =
            fixture()["exit"]!!.jsonArray.map { it.jsonObject }.single {
                it["name"]!!.jsonPrimitive.content == "weights_are_compared_never_summed"
            }
        val relaysJson =
            case["candidates"]!!.jsonArray.joinToString(",", "[", "]") { c ->
                val candidate = c.jsonObject
                """{"exit_id":${candidate["exit_id"]},"exit_pubkey_hex":"","endpoint":"","country":"XX","city":"","active":true,"weight":${candidate["weight"]}}"""
            }
        val answer = com.warrenbrowse.vpn.jni.WarrenJni.resolveExitPin("""{"kind":"country","country":"xx"}""", relaysJson)
        assertEquals(case["expected"]!!.jsonPrimitive.longOrNull?.toInt(), JniExitPinResolver.decodeIndex(answer))
    }

    @Test
    fun a_failover_through_the_native_resolver_never_returns_the_failed_exit() {
        val a = row(0, "00000000000000000000000000000001", 10).copy(exitPubkeyHex = "aa")
        val b = row(1, "00000000000000000000000000000002", 30).copy(exitPubkeyHex = "bb")
        assertEquals(a, resolver.failover(ExitPin.Automatic, null, listOf(a, b), "bb"))
        assertNull(resolver.failover(ExitPin.Exit(b.exitId), null, listOf(a, b), "bb"))
    }
}
