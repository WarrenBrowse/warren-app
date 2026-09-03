package com.warrenbrowse.vpn.app.connect

import com.warrenbrowse.vpn.lib.repository.ExitChoice
import com.warrenbrowse.vpn.lib.repository.ExitPin
import com.warrenbrowse.vpn.lib.repository.WarrenRelaySummary
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Test

/**
 * The Kotlin half of the exit-choice JNI contract: the exact bytes the resolver
 * hands `WarrenJni.resolveExitPin` / `resolveAutomaticExit` /
 * `resolveFailoverExit` and how it reads the answer. The Rust half (`warren-jni/src/exit_pin.rs`, `the_json_contract_*`)
 * pins the same request literals to the same answers, and the instrumented
 * `ExitPinJniTest` replays the shared `exit_pick.json` vector through the real
 * library on a device.
 */
class JniExitPinResolverTest {
    private val frankfurt =
        WarrenRelaySummary(
            exitId = "00000000000000000000000000000001",
            exitPubkeyHex = "aa",
            endpoint = "10.0.0.1:443",
            country = "DE",
            city = "Frankfurt",
            active = true,
            weight = 10,
        )
    private val berlin =
        frankfurt.copy(
            exitId = "00000000000000000000000000000002",
            exitPubkeyHex = "bb",
            endpoint = "10.0.0.2:443",
            city = "Berlin",
            weight = 30,
        )
    private val rows = listOf(frankfurt, berlin)

    /** The literal the Rust test `TWO_GERMAN_ROWS` decodes. */
    private val twoGermanRows =
        """[{"exit_id":"00000000000000000000000000000001","exit_pubkey_hex":"aa","endpoint":"10.0.0.1:443","country":"DE","city":"Frankfurt","active":true,"weight":10},""" +
            """{"exit_id":"00000000000000000000000000000002","exit_pubkey_hex":"bb","endpoint":"10.0.0.2:443","country":"DE","city":"Berlin","active":true,"weight":30}]"""

    @Test
    fun `a pin crosses the boundary as its kind tag and fields`() {
        assertEquals("""{"kind":"automatic"}""", JniExitPinResolver.encodePin(ExitPin.Automatic))
        assertEquals(
            """{"kind":"country","country":"DE"}""",
            JniExitPinResolver.encodePin(ExitPin.Country("DE")),
        )
        assertEquals(
            """{"kind":"city","country":"de","city":"frankfurt"}""",
            JniExitPinResolver.encodePin(ExitPin.City("de", "frankfurt")),
        )
        assertEquals(
            """{"kind":"exit","exit_id":"00000000000000000000000000000002"}""",
            JniExitPinResolver.encodePin(ExitPin.Exit("00000000000000000000000000000002")),
        )
    }

    @Test
    fun `the snapshot crosses the boundary in the listRelays schema`() {
        assertEquals(twoGermanRows, JniExitPinResolver.encodeRelays(rows))
    }

    @Test
    fun `the answer names a position in the list that was sent`() {
        assertEquals(1, JniExitPinResolver.decodeIndex("""{"index":1}"""))
        assertNull(JniExitPinResolver.decodeIndex("""{"index":null}"""))
    }

    @Test
    fun `an answer outside the contract is a failure, never an empty scope`() {
        // A native library and a Kotlin decoder from two revisions must not
        // look like "nothing fits": the caller widens the scope on that answer.
        for (offContract in listOf("not json", """{"exit":"bb"}""", """{"index":"one"}""")) {
            val resolver = JniExitPinResolver(resolveJson = { _, _ -> offContract })
            assertEquals(
                ExitChoice.ResolverFailed,
                resolver.resolve(ExitPin.Country("DE"), rows),
                "answer: $offContract",
            )
        }
        val past = JniExitPinResolver(resolveJson = { _, _ -> """{"index":7}""" })
        assertEquals(
            ExitChoice.ResolverFailed,
            past.resolve(ExitPin.Country("DE"), rows),
            "a position past the list is not an empty scope either",
        )
    }

    @Test
    fun `the rule's own refusal stays an empty scope`() {
        val resolver =
            JniExitPinResolver(
                resolveJson = { _, _ -> """{"index":null}""" },
                automaticJson = { _, _ -> """{"index":null}""" },
                failoverJson = { _, _, _, _ -> """{"index":null}""" },
            )
        assertEquals(ExitChoice.NoneInScope, resolver.resolve(ExitPin.Country("DE"), rows))
        assertEquals(ExitChoice.NoneInScope, resolver.automatic(null, rows))
        assertEquals(ExitChoice.NoneInScope, resolver.failover(ExitPin.Automatic, null, rows, "bb"))
    }

    @Test
    fun `resolve sends the pin and the snapshot and maps the answer back to a relay`() {
        var sent: Pair<String, String>? = null
        val resolver =
            JniExitPinResolver(
                resolveJson = { pin, relays ->
                    sent = pin to relays
                    """{"index":1}"""
                }
            )
        assertEquals(ExitChoice.Picked(berlin), resolver.resolve(ExitPin.Country("DE"), rows))
        assertEquals("""{"kind":"country","country":"DE"}""" to twoGermanRows, sent)
    }

    @Test
    fun `automatic sends the preferred country and the snapshot, with no pin`() {
        var sent: Pair<String, String>? = null
        val resolver =
            JniExitPinResolver(
                automaticJson = { exitCountry, relays ->
                    sent = exitCountry to relays
                    """{"index":1}"""
                }
            )
        assertEquals(ExitChoice.Picked(berlin), resolver.automatic("DE", rows))
        assertEquals("DE" to twoGermanRows, sent)
        resolver.automatic(null, rows)
        assertEquals("", sent?.first, "no preference travels as the empty string")
    }

    @Test
    fun `failover sends the pin the preferred country the snapshot and the failed key`() {
        var sent: List<String>? = null
        val resolver =
            JniExitPinResolver(
                failoverJson = { pin, exitCountry, relays, failed ->
                    sent = listOf(pin, exitCountry, relays, failed)
                    """{"index":0}"""
                }
            )
        assertEquals(ExitChoice.Picked(frankfurt), resolver.failover(ExitPin.Automatic, null, rows, "bb"))
        assertEquals(listOf("""{"kind":"automatic"}""", "", twoGermanRows, "bb"), sent)
        resolver.failover(ExitPin.Automatic, "FR", rows, "bb")
        assertEquals("FR", sent?.get(1), "a preferred exit country travels as itself")
    }

    @Test
    fun `a native call that throws is a failure the caller can refuse to dial on`() {
        val resolver =
            JniExitPinResolver(
                resolveJson = { _, _ -> throw UnsatisfiedLinkError("no library") },
                automaticJson = { _, _ -> throw UnsatisfiedLinkError("no library") },
                failoverJson = { _, _, _, _ -> throw IllegalStateException("boom") },
            )
        assertEquals(ExitChoice.ResolverFailed, resolver.resolve(ExitPin.Country("DE"), rows))
        assertEquals(ExitChoice.ResolverFailed, resolver.automatic("DE", rows))
        assertEquals(ExitChoice.ResolverFailed, resolver.failover(ExitPin.Automatic, null, rows, "bb"))
    }
}
