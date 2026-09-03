package com.warrenbrowse.vpn.app.forum

import com.warrenbrowse.vpn.lib.model.forum.ForumHeaderButton
import com.warrenbrowse.vpn.lib.repository.ForumActivityState
import com.warrenbrowse.vpn.lib.repository.WarrenConnectedInfo
import kotlin.time.Duration.Companion.seconds
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.runTest
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Test

@OptIn(ExperimentalCoroutinesApi::class)
class ForumDigestPollerTest {

    private class RecordingActivity : ForumActivityState {
        override val unread: StateFlow<Int> = MutableStateFlow(0)
        override val headerButton: StateFlow<ForumHeaderButton> = MutableStateFlow(ForumHeaderButton.NONE)
        val digests = mutableListOf<String?>()

        override fun setDigest(counts: String?) {
            digests += counts
        }

        override fun setObservedUnread(unread: Int) = Unit
    }

    @Test
    fun a_reachable_server_is_polled_once_a_minute_whatever_it_said() {
        // The daemon's cadence: a 304, a fresh document and a refused one all
        // clear the fast retry.
        assertEquals(60.seconds to null, ForumDigestCadence.next("ok", null))
        assertEquals(60.seconds to null, ForumDigestCadence.next("not-modified", 40.seconds))
        assertEquals(60.seconds to null, ForumDigestCadence.next("rejected", 20.seconds))
    }

    @Test
    fun a_transport_failure_retries_early_doubling_up_to_the_ceiling() {
        // A client that just woke or regained a network must not sit a full
        // interval with a badge it can no longer justify.
        assertEquals(20.seconds to 20.seconds, ForumDigestCadence.next("transport", null))
        assertEquals(40.seconds to 40.seconds, ForumDigestCadence.next("transport", 20.seconds))
        assertEquals(45.seconds to 45.seconds, ForumDigestCadence.next("transport", 40.seconds))
        assertEquals(45.seconds to 45.seconds, ForumDigestCadence.next("transport", 45.seconds))
        assertEquals(20.seconds to 20.seconds, ForumDigestCadence.next("deferred", null))
    }

    @Test
    fun one_fetch_hands_the_verified_counts_or_their_absence_to_the_monitor() = runTest {
        val activity = RecordingActivity()
        var answer = """{"counts":"03f","fetch":"ok"}"""
        val jni = FakeJniBridge(digestAnswer = { answer })
        val poller = ForumDigestPoller(jni, activity, FakeTunnelStateProvider(), UnconfinedTestDispatcher(testScheduler))

        assertEquals("ok", poller.fetchOnce())
        answer = """{"counts":null,"fetch":"transport"}"""
        assertEquals("transport", poller.fetchOnce())

        // An absent document is unknown, never zero: the monitor gets null.
        assertEquals(listOf("03f", null), activity.digests)
        assertEquals(2, jni.digestCalls)
    }

    @Test
    fun a_tunnel_between_states_defers_the_fetch_and_keeps_the_last_document() = runTest {
        val activity = RecordingActivity()
        val jni = FakeJniBridge()
        val tunnel = FakeTunnelStateProvider(WarrenConnectedInfo.Blocking("x"))
        val poller = ForumDigestPoller(jni, activity, tunnel, UnconfinedTestDispatcher(testScheduler))

        assertEquals("deferred", poller.fetchOnce())

        assertEquals(0, jni.digestCalls)
        assertEquals(emptyList<String?>(), activity.digests)
    }

    @Test
    fun a_malformed_envelope_reads_as_a_transport_failure() {
        assertEquals(null to "transport", parseDigestEnvelope("nope"))
        assertEquals("00" to "not-modified", parseDigestEnvelope("""{"counts":"00","fetch":"not-modified"}"""))
        assertEquals(null to "rejected", parseDigestEnvelope("""{"counts":null,"fetch":"rejected"}"""))
    }
}
