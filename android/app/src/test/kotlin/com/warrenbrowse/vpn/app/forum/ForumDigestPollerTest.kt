package com.warrenbrowse.vpn.app.forum

import com.warrenbrowse.vpn.lib.model.forum.ForumHeaderButton
import com.warrenbrowse.vpn.lib.model.forum.ForumIdentity
import com.warrenbrowse.vpn.lib.repository.ForumActivityState
import com.warrenbrowse.vpn.lib.repository.WarrenConnectedInfo
import kotlin.time.Duration.Companion.minutes
import kotlin.time.Duration.Companion.seconds
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.runCurrent
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
    fun the_poll_runs_only_while_the_digest_is_wanted() = runTest {
        val activity = RecordingActivity()
        val jni = FakeJniBridge(digestAnswer = { """{"counts":"000","fetch":"ok"}""" })
        val poller =
            ForumDigestPoller(jni, activity, FakeTunnelStateProvider(), UnconfinedTestDispatcher(testScheduler))
        val wanted = MutableStateFlow(false)
        val loop = launch { poller.runWhile(wanted) }

        advanceTimeBy(5.minutes)
        assertEquals(0, jni.digestCalls, "nothing is fetched for a digest nobody reads")

        wanted.value = true
        runCurrent()
        assertEquals(1, jni.digestCalls, "the poll starts with a fetch the moment it is wanted")
        advanceTimeBy(61.seconds)
        assertEquals(2, jni.digestCalls, "then one a minute")

        wanted.value = false
        runCurrent()
        advanceTimeBy(5.minutes)
        assertEquals(2, jni.digestCalls, "the poll stops the moment the digest has no reader")
        loop.cancel()
    }

    @Test
    fun the_digest_is_wanted_only_with_consent_the_setting_and_a_forum_account() = runTest {
        val accepted = MutableStateFlow(true)
        val enabled = MutableStateFlow(true)
        val identity = MutableStateFlow<ForumIdentity?>(ForumIdentity("lusab-babad-dovok", 2))
        val wanted = forumDigestWanted(accepted, enabled, identity)
        assertEquals(true, wanted.first())

        enabled.value = false
        assertEquals(false, wanted.first(), "off hides the bell, so nothing would show the count")
        enabled.value = true
        identity.value = null
        assertEquals(false, wanted.first(), "no forum account, nothing to index the digest by")
        identity.value = ForumIdentity("lusab-babad-dovok", 2)
        accepted.value = false
        assertEquals(false, wanted.first(), "nothing leaves the device before the disclosure")
    }

    @Test
    fun a_malformed_envelope_reads_as_a_transport_failure() {
        assertEquals(null to "transport", parseDigestEnvelope("nope"))
        assertEquals("00" to "not-modified", parseDigestEnvelope("""{"counts":"00","fetch":"not-modified"}"""))
        assertEquals(null to "rejected", parseDigestEnvelope("""{"counts":null,"fetch":"rejected"}"""))
    }
}
