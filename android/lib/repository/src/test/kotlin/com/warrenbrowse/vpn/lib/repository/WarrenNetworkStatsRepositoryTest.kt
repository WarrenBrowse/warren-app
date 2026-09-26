package com.warrenbrowse.vpn.lib.repository

import com.warrenbrowse.vpn.lib.model.NetworkStatsFetch
import com.warrenbrowse.vpn.lib.model.WarrenNetworkStatsParser
import kotlin.time.Duration.Companion.minutes
import kotlin.time.Duration.Companion.seconds
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Test

@OptIn(ExperimentalCoroutinesApi::class)
class WarrenNetworkStatsRepositoryTest {

    private fun snapshot(generatedAt: Long, windowSecs: Int = 60): String =
        """
        {"version":1,"environment":"beta","generated_at":$generatedAt,"window_secs":$windowSecs,
         "exit_users_rounding":5,"exit_live_threshold":20,
         "users":{"accounts_total":10,"subscribers_active":8,"connected":3},
         "fleet":{"exits_online":1,"exits_total":1,"download_bps":1,"upload_bps":1,
                  "capacity_bps":1000,"load_percent":4,"load_level":"low",
                  "transferred_24h_bytes":0,"peak_connected_24h":3,"peak_throughput_24h_bps":2},
         "exits":[],"history":[]}
        """

    private fun ok(generatedAt: Long, windowSecs: Int = 60) =
        """{"ok":true,"stats":${snapshot(generatedAt, windowSecs)}}"""

    private val unavailable = """{"ok":false,"reason":"unavailable"}"""
    private val transport = """{"ok":false,"reason":"transport"}"""

    /** Answers from [answers] in order, repeating the last one. */
    private class ScriptedBridge(private vararg val answers: () -> String) :
        WarrenJniBridge by NoBridge {
        var calls = 0

        override fun fetchNetworkStats(): String {
            val answer = answers[calls.coerceAtMost(answers.lastIndex)]
            calls++
            return answer()
        }
    }

    private fun TestScope.repository(bridge: WarrenJniBridge, deferred: () -> Boolean = { false }) =
        WarrenNetworkStatsRepository(
            bridge = bridge,
            scope = backgroundScope,
            io = StandardTestDispatcher(testScheduler),
            timeSource = testScheduler.timeSource,
            deferred = deferred,
        )

    private fun TestScope.watch(repository: WarrenNetworkStatsRepository): Job =
        backgroundScope.launch {
            repository.state.collect {}
        }

    @Test
    fun `nothing is fetched while no surface watches`() = runTest {
        val bridge = ScriptedBridge({ ok(1_000) })
        repository(bridge)

        advanceTimeBy(30.minutes)
        runCurrent()

        assertEquals(0, bridge.calls)
    }

    @Test
    fun `a surface that starts watching gets a fetch at once`() = runTest {
        val bridge = ScriptedBridge({ ok(1_000) })
        val repository = repository(bridge)

        watch(repository)
        runCurrent()

        assertEquals(1, bridge.calls)
        assertEquals(1_000L, repository.state.value.snapshot?.generatedAt)
        assertEquals(NetworkStatsAvailability.AVAILABLE, repository.state.value.availability)
    }

    @Test
    fun `a watched feed fetches once per window`() = runTest {
        val bridge = ScriptedBridge({ ok(1_000) }, { ok(1_060) })
        val repository = repository(bridge)
        watch(repository)
        runCurrent()

        advanceTimeBy(59.seconds)
        runCurrent()
        assertEquals(1, bridge.calls, "no second fetch inside the window")

        advanceTimeBy(1.seconds)
        runCurrent()
        assertEquals(2, bridge.calls)
        assertEquals(1_060L, repository.state.value.snapshot?.generatedAt)
    }

    @Test
    fun `polling stops when the last surface stops watching`() = runTest {
        val bridge = ScriptedBridge({ ok(1_000) })
        val repository = repository(bridge)
        val surface = watch(repository)
        runCurrent()

        surface.cancel()
        advanceTimeBy(30.minutes)
        runCurrent()

        assertEquals(1, bridge.calls)
    }

    @Test
    fun `a surface that comes back inside the window does not refetch`() = runTest {
        val bridge = ScriptedBridge({ ok(1_000) })
        val repository = repository(bridge)
        val first = watch(repository)
        runCurrent()
        first.cancel()

        advanceTimeBy(30.seconds)
        watch(repository)
        runCurrent()
        assertEquals(1, bridge.calls, "the window of the first fetch is still open")

        advanceTimeBy(30.seconds)
        runCurrent()
        assertEquals(2, bridge.calls)
    }

    @Test
    fun `a failure keeps the last good snapshot on screen`() = runTest {
        val bridge = ScriptedBridge({ ok(1_000) }, { transport })
        val repository = repository(bridge)
        watch(repository)
        runCurrent()

        advanceTimeBy(60.seconds)
        runCurrent()

        assertEquals(2, bridge.calls)
        assertEquals(1_000L, repository.state.value.snapshot?.generatedAt)
        assertEquals(NetworkStatsAvailability.FAILING, repository.state.value.availability)
    }

    @Test
    fun `an API without the endpoint is asked again after minutes, then less often`() = runTest {
        val bridge = ScriptedBridge({ unavailable })
        val repository = repository(bridge)
        watch(repository)
        runCurrent()
        assertNull(repository.state.value.snapshot)
        assertEquals(NetworkStatsAvailability.UNAVAILABLE, repository.state.value.availability)

        advanceTimeBy(10.minutes - 1.seconds)
        runCurrent()
        assertEquals(1, bridge.calls)
        advanceTimeBy(1.seconds)
        runCurrent()
        assertEquals(2, bridge.calls)

        advanceTimeBy(20.minutes - 1.seconds)
        runCurrent()
        assertEquals(2, bridge.calls, "the second wait doubled")
        advanceTimeBy(1.seconds)
        runCurrent()
        assertEquals(3, bridge.calls)
    }

    @Test
    fun `a transient failure retries soon, doubling`() = runTest {
        val bridge = ScriptedBridge({ transport })
        val repository = repository(bridge)
        watch(repository)
        runCurrent()

        advanceTimeBy(15.seconds)
        runCurrent()
        assertEquals(2, bridge.calls)
        advanceTimeBy(29.seconds)
        runCurrent()
        assertEquals(2, bridge.calls)
        advanceTimeBy(1.seconds)
        runCurrent()
        assertEquals(3, bridge.calls)
    }

    @Test
    fun `a success after failures returns to the window cadence`() {
        val (wait, backoff) =
            NetworkStatsCadence.next(
                NetworkStatsFetch.Snapshot(
                    WarrenNetworkStatsParser.parse(snapshot(1_000, windowSecs = 120))!!
                ),
                backoff = 4.minutes,
            )

        assertEquals(120.seconds, wait)
        assertNull(backoff)
    }

    @Test
    fun `an older snapshot never replaces a newer one`() = runTest {
        val bridge = ScriptedBridge({ ok(1_060) }, { ok(1_000) })
        val repository = repository(bridge)
        watch(repository)
        runCurrent()

        advanceTimeBy(60.seconds)
        runCurrent()

        assertEquals(1_060L, repository.state.value.snapshot?.generatedAt)
    }

    @Test
    fun `a bridge that throws is a failure, not a crash`() = runTest {
        val bridge = ScriptedBridge({ error("native library gone") })
        val repository = repository(bridge)
        watch(repository)
        runCurrent()

        assertEquals(NetworkStatsAvailability.FAILING, repository.state.value.availability)
    }

    @Test
    fun `a tunnel between states defers the fetch without reaching the bridge`() = runTest {
        var between = true
        val bridge = ScriptedBridge({ ok(1_000) })
        val repository = repository(bridge, deferred = { between })
        watch(repository)
        runCurrent()
        assertEquals(0, bridge.calls)

        between = false
        advanceTimeBy(15.seconds)
        runCurrent()
        assertEquals(1, bridge.calls)
    }
}

/** A bridge whose every method is unused by the network stats feed. */
private object NoBridge : WarrenJniBridge {
    override fun generateMnemonic(): String = error("unused")

    override fun mnemonicPubkeySs58(mnemonic: String): String = error("unused")

    override fun fetchVersionInfo(currentVersion: String): WarrenVersionVerdict = error("unused")

    override fun fetchNetworkInfo(): String = error("unused")

    override fun fetchNetworkStats(): String = error("unused")

    override fun forumLogin(mnemonic: String, sid: String, host: String): String = error("unused")

    override fun forumLoginCancel(sid: String, host: String) = error("unused")

    override fun forumAttachLogs(
        mnemonic: String,
        sid: String,
        topicId: Long,
        host: String,
        logGz: ByteArray,
    ): String = error("unused")

    override fun forumAttachCancel(sid: String, host: String) = error("unused")

    override fun forumCodeProbe(sid: String, host: String, budgetMillis: Long): String =
        error("unused")

    override fun forumReport(mnemonic: String, reportJson: String, logGz: ByteArray?): String =
        error("unused")

    override fun forumDigestFetch(): String = error("unused")

    override fun noticesFetch(currentVersion: String): String = error("unused")

    override fun announcementsFetch(currentVersion: String): String = error("unused")

    override fun campaignVoucher(mnemonic: String, campaignId: String): String = error("unused")

    override fun forumNotifications(mnemonic: String): String = error("unused")

    override fun forumNotificationsSeen(mnemonic: String): String = error("unused")

    override fun reportPubkeyMismatch(
        mnemonic: String,
        exitIdHex: String,
        oldPubkeyHex: String,
        newPubkeyHex: String,
        countryCode: String,
        city: String,
    ): String = error("unused")

    override fun collectProblemReport(
        metadataJson: String,
        redactJson: String,
        appLogDir: String,
        outputPath: String,
        forSend: Boolean,
    ): String = error("unused")
}
