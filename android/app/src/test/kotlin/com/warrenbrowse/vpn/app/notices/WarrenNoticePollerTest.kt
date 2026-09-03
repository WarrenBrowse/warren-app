package com.warrenbrowse.vpn.app.notices

import com.warrenbrowse.vpn.app.forum.FakeJniBridge
import com.warrenbrowse.vpn.app.forum.FakeTunnelStateProvider
import com.warrenbrowse.vpn.lib.model.WarrenNotice
import com.warrenbrowse.vpn.lib.model.WarrenNoticeLevel
import com.warrenbrowse.vpn.lib.repository.WarrenConnectedInfo
import com.warrenbrowse.vpn.lib.repository.WarrenNoticeRepository
import kotlin.time.Duration.Companion.minutes
import kotlin.time.Duration.Companion.seconds
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

private const val VERSION = "1.1.23"

@OptIn(ExperimentalCoroutinesApi::class)
class WarrenNoticePollerTest {

    private fun poller(
        jni: FakeJniBridge,
        state: WarrenNoticeRepository,
        tunnel: FakeTunnelStateProvider = FakeTunnelStateProvider(),
        scheduler: kotlinx.coroutines.test.TestCoroutineScheduler,
    ) = WarrenNoticePoller(jni, state, tunnel, VERSION, UnconfinedTestDispatcher(scheduler))

    @Test
    fun a_reachable_server_is_polled_every_five_minutes_whatever_it_said() {
        // The daemon's cadence: a fresh envelope and a refused one both clear
        // the fast retry, because both prove the server is reachable.
        assertEquals(5.minutes to null, WarrenNoticeCadence.next("ok", null))
        assertEquals(5.minutes to null, WarrenNoticeCadence.next("rejected", 40.seconds))
    }

    @Test
    fun a_transport_failure_retries_early_doubling_up_to_the_ceiling() {
        // A client that just regained a network must not sit a full interval
        // showing nothing, or showing a notice whose envelope is about to lapse.
        assertEquals(20.seconds to 20.seconds, WarrenNoticeCadence.next("transport", null))
        assertEquals(40.seconds to 40.seconds, WarrenNoticeCadence.next("transport", 20.seconds))
        assertEquals(240.seconds to 240.seconds, WarrenNoticeCadence.next("transport", 160.seconds))
        assertEquals(240.seconds to 240.seconds, WarrenNoticeCadence.next("transport", 240.seconds))
        assertEquals(20.seconds to 20.seconds, WarrenNoticeCadence.next("deferred", null))
        assertTrue(
            WarrenNoticeCadence.RETRY_MAX < WarrenNoticeCadence.CHECK_INTERVAL,
            "a long outage must degrade into the normal cadence, never into silence",
        )
    }

    @Test
    fun one_fetch_publishes_the_verified_notices_with_this_build_version() = runTest {
        val state = WarrenNoticeRepository()
        val jni =
            FakeJniBridge(
                noticesAnswer = {
                    """{"notices":[{"id":"a1","message":"exit outage in NL","level":"error"}],""" +
                        """"fetch":"ok"}"""
                }
            )

        assertEquals("ok", poller(jni, state, scheduler = testScheduler).fetchOnce())

        assertEquals(
            listOf(WarrenNotice("a1", "exit outage in NL", WarrenNoticeLevel.ERROR)),
            state.notices.value,
        )
        assertEquals(
            listOf(VERSION),
            jni.noticesVersions,
            "the range filter runs in Rust, so it must be told which version is asking",
        )
    }

    @Test
    fun an_erased_notice_clears_the_banner_from_the_same_signal_that_raised_it() = runTest {
        val state = WarrenNoticeRepository()
        var answer = """{"notices":[{"id":"a1","message":"live","level":"info"}],"fetch":"ok"}"""
        val jni = FakeJniBridge(noticesAnswer = { answer })
        val poller = poller(jni, state, scheduler = testScheduler)
        poller.fetchOnce()

        answer = """{"notices":[],"fetch":"ok"}"""
        poller.fetchOnce()

        assertEquals(emptyList<WarrenNotice>(), state.notices.value)
    }

    @Test
    fun an_unreadable_envelope_leaves_the_last_notice_standing() = runTest {
        // A parsing bug must never erase a live operator message: only the
        // server, or the signed expiry, takes a banner down.
        val state = WarrenNoticeRepository()
        var answer = """{"notices":[{"id":"a1","message":"live","level":"warning"}],"fetch":"ok"}"""
        val jni = FakeJniBridge(noticesAnswer = { answer })
        val poller = poller(jni, state, scheduler = testScheduler)
        poller.fetchOnce()

        answer = "not json at all"
        assertEquals("transport", poller.fetchOnce())

        assertEquals(
            listOf(WarrenNotice("a1", "live", WarrenNoticeLevel.WARNING)),
            state.notices.value,
        )
    }

    @Test
    fun a_row_without_a_message_is_dropped_and_an_unknown_severity_reads_as_info() = runTest {
        val state = WarrenNoticeRepository()
        val jni =
            FakeJniBridge(
                noticesAnswer = {
                    """{"notices":[{"id":"a1","message":"","level":"error"},""" +
                        """{"id":"a2","message":"still shown","level":"whatever"}],"fetch":"ok"}"""
                }
            )

        poller(jni, state, scheduler = testScheduler).fetchOnce()

        assertEquals(
            listOf(WarrenNotice("a2", "still shown", WarrenNoticeLevel.INFO)),
            state.notices.value,
        )
    }

    @Test
    fun a_tunnel_between_states_defers_the_fetch_and_keeps_the_last_notice() = runTest {
        val state = WarrenNoticeRepository()
        val jni = FakeJniBridge()
        val tunnel = FakeTunnelStateProvider(WarrenConnectedInfo.Blocking("x"))

        val fetch = poller(jni, state, tunnel, testScheduler).fetchOnce()

        assertEquals("deferred", fetch)
        assertEquals(0, jni.noticesCalls, "the request would hang in a TUN that is coming up")
        assertEquals(emptyList<WarrenNotice>(), state.notices.value)
    }

    @Test
    fun nothing_is_fetched_before_the_privacy_disclosure_is_accepted() = runTest {
        // The lifecycle gate: nothing leaves this device before the user has
        // accepted the disclosure, and a broadcast banner is no exception.
        val state = WarrenNoticeRepository()
        val jni = FakeJniBridge()
        val accepted = MutableStateFlow(false)
        val job = launch { poller(jni, state, scheduler = testScheduler).runWhile(accepted) }

        advanceTimeBy(10.minutes)
        runCurrent()
        assertEquals(0, jni.noticesCalls)

        accepted.value = true
        runCurrent()
        assertEquals(1, jni.noticesCalls, "accepting must fetch at once, not a poll later")

        job.cancel()
    }

    @Test
    fun the_poll_stops_with_the_window_and_resumes_with_a_fetch() = runTest {
        val state = WarrenNoticeRepository()
        val jni = FakeJniBridge(noticesAnswer = { """{"notices":[],"fetch":"ok"}""" })
        val accepted = MutableStateFlow(true)
        val job = launch { poller(jni, state, scheduler = testScheduler).runWhile(accepted) }
        runCurrent()
        assertEquals(1, jni.noticesCalls)

        advanceTimeBy(5.minutes + 1.seconds)
        runCurrent()
        assertEquals(2, jni.noticesCalls, "one poll every five minutes while it runs")

        job.cancel()
        runCurrent()
        advanceTimeBy(30.minutes)
        runCurrent()
        assertEquals(2, jni.noticesCalls, "a cancelled scope must poll nothing at all")
    }

    @Test
    fun a_throwing_boundary_is_one_failed_fetch_rather_than_a_crash() = runTest {
        val state = WarrenNoticeRepository()
        val jni = FakeJniBridge(noticesAnswer = { error("the library is not loaded") })

        assertEquals("transport", poller(jni, state, scheduler = testScheduler).fetchOnce())
        assertEquals(emptyList<WarrenNotice>(), state.notices.value)
    }

    @Test
    fun the_envelope_parser_reads_the_fetch_class_and_the_rows() {
        val (notices, fetch) =
            parseNoticesEnvelope(
                """{"notices":[{"id":"a1","message":"one \" quote","level":"warning"}],""" +
                    """"fetch":"rejected"}"""
            )

        assertEquals("rejected", fetch)
        assertEquals(listOf(WarrenNotice("a1", """one " quote""", WarrenNoticeLevel.WARNING)), notices)
    }

    @Test
    fun the_envelope_parser_reports_an_unreadable_answer_as_no_answer_at_all() {
        val (notices, fetch) = parseNoticesEnvelope("""{"notices":42}""")

        assertNull(notices, "an empty set and an unreadable answer must not look the same")
        assertEquals("transport", fetch)
    }
}
