package com.warrenbrowse.vpn.app.announcements

import com.warrenbrowse.vpn.app.forum.FakeJniBridge
import com.warrenbrowse.vpn.app.forum.FakeTunnelStateProvider
import com.warrenbrowse.vpn.app.forum.FakeWalletRepository
import com.warrenbrowse.vpn.lib.model.WarrenAnnouncement
import com.warrenbrowse.vpn.lib.model.WarrenAnnouncementCta
import com.warrenbrowse.vpn.lib.model.WarrenNoticeLevel
import com.warrenbrowse.vpn.lib.model.wallet.WalletState
import com.warrenbrowse.vpn.lib.repository.WarrenAnnouncementRepository
import com.warrenbrowse.vpn.lib.repository.WarrenConnectedInfo
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

private const val LAUNCH_CARD =
    """{"announcements":[{"id":"a1","headline":"Production is open",""" +
        """"body":"Your beta account gets a free month.","level":"warning",""" +
        """"cta":{"label":"Get Warren","url":"https://warren.ro/download"},""" +
        """"voucher_campaign_id":"prod-launch"}],"fetch":"ok"}"""

private const val PLAIN_CARD =
    """{"announcements":[{"id":"a1","headline":"Production is open","body":"Read more.",""" +
        """"level":"info","cta":null,"voucher_campaign_id":null}],"fetch":"ok"}"""

@OptIn(ExperimentalCoroutinesApi::class)
class WarrenAnnouncementPollerTest {

    private fun poller(
        jni: FakeJniBridge,
        state: WarrenAnnouncementRepository,
        wallet: FakeWalletRepository = FakeWalletRepository(),
        tunnel: FakeTunnelStateProvider = FakeTunnelStateProvider(),
        scheduler: kotlinx.coroutines.test.TestCoroutineScheduler,
    ) =
        WarrenAnnouncementPoller(
            jni,
            state,
            tunnel,
            wallet,
            VERSION,
            UnconfinedTestDispatcher(scheduler),
        )

    @Test
    fun a_reachable_server_is_polled_every_five_minutes_whatever_it_said() {
        // The daemon's cadence: a fresh envelope and a refused one both clear
        // the fast retry, because both prove the server is reachable.
        assertEquals(5.minutes to null, WarrenAnnouncementCadence.next("ok", null))
        assertEquals(5.minutes to null, WarrenAnnouncementCadence.next("rejected", 40.seconds))
    }

    @Test
    fun a_transport_failure_retries_early_doubling_up_to_the_ceiling() {
        assertEquals(20.seconds to 20.seconds, WarrenAnnouncementCadence.next("transport", null))
        assertEquals(
            40.seconds to 40.seconds,
            WarrenAnnouncementCadence.next("transport", 20.seconds),
        )
        assertEquals(
            240.seconds to 240.seconds,
            WarrenAnnouncementCadence.next("transport", 160.seconds),
        )
        assertEquals(20.seconds to 20.seconds, WarrenAnnouncementCadence.next("deferred", null))
        assertTrue(
            WarrenAnnouncementCadence.RETRY_MAX < WarrenAnnouncementCadence.CHECK_INTERVAL,
            "a long outage must degrade into the normal cadence, never into silence",
        )
    }

    @Test
    fun one_fetch_publishes_the_verified_card_with_this_account_code() = runTest {
        val state = WarrenAnnouncementRepository()
        val wallet = FakeWalletRepository()
        val jni =
            FakeJniBridge(
                announcementsAnswer = { LAUNCH_CARD },
                voucherAnswer = { """{"ok":true,"code":"ABCD1234EFGH5678"}""" },
            )

        assertEquals("ok", poller(jni, state, wallet, scheduler = testScheduler).fetchOnce())

        assertEquals(
            listOf(
                WarrenAnnouncement(
                    id = "a1",
                    headline = "Production is open",
                    body = "Your beta account gets a free month.",
                    level = WarrenNoticeLevel.WARNING,
                    cta = WarrenAnnouncementCta("Get Warren", "https://warren.ro/download"),
                    voucherCampaignId = "prod-launch",
                    voucherCode = "ABCD1234EFGH5678",
                )
            ),
            state.announcements.value,
        )
        assertEquals(
            listOf(VERSION),
            jni.announcementsVersions,
            "the range filter runs in Rust, so it must be told which version is asking",
        )
        assertEquals(listOf("prod-launch"), jni.voucherCampaigns)
    }

    @Test
    fun an_announcement_with_no_campaign_never_asks_for_a_code() = runTest {
        // The signed lookup is the one request the offer makes that is tied to
        // an account. An announcement carrying no offer must not make it, and
        // must not read the wallet to find that out.
        val state = WarrenAnnouncementRepository()
        val wallet = FakeWalletRepository()
        val jni = FakeJniBridge(announcementsAnswer = { PLAIN_CARD })

        poller(jni, state, wallet, scheduler = testScheduler).fetchOnce()

        assertEquals(emptyList<String>(), jni.voucherCampaigns)
        assertEquals(0, wallet.mnemonicReads, "no offer, no reason to touch the wallet")
        assertNull(state.announcements.value.single().voucherCode)
    }

    @Test
    fun a_device_with_no_wallet_still_reads_the_announcement() = runTest {
        val state = WarrenAnnouncementRepository()
        val wallet = FakeWalletRepository(WalletState.Absent)
        val jni = FakeJniBridge(announcementsAnswer = { LAUNCH_CARD })

        poller(jni, state, wallet, scheduler = testScheduler).fetchOnce()

        assertEquals(emptyList<String>(), jni.voucherCampaigns)
        assertEquals(
            "Production is open",
            state.announcements.value.single().headline,
            "the operator's text is not what the offer is for",
        )
        assertNull(state.announcements.value.single().voucherCode)
    }

    @Test
    fun an_account_outside_the_cohort_reads_the_announcement_without_a_code() = runTest {
        val state = WarrenAnnouncementRepository()
        val jni =
            FakeJniBridge(
                announcementsAnswer = { LAUNCH_CARD },
                voucherAnswer = { """{"ok":true,"code":null}""" },
            )

        poller(jni, state, scheduler = testScheduler).fetchOnce()

        assertNull(state.announcements.value.single().voucherCode)
    }

    @Test
    fun a_withdrawn_announcement_clears_the_card_from_the_signal_that_raised_it() = runTest {
        val state = WarrenAnnouncementRepository()
        var answer = PLAIN_CARD
        val jni = FakeJniBridge(announcementsAnswer = { answer })
        val poller = poller(jni, state, scheduler = testScheduler)
        poller.fetchOnce()

        answer = """{"announcements":[],"fetch":"ok"}"""
        poller.fetchOnce()

        assertEquals(emptyList<WarrenAnnouncement>(), state.announcements.value)
    }

    @Test
    fun an_unreadable_envelope_leaves_the_last_announcement_standing() = runTest {
        // A parsing bug must never erase a live announcement: only the server,
        // or the signed expiry, takes a card down.
        val state = WarrenAnnouncementRepository()
        var answer = PLAIN_CARD
        val jni = FakeJniBridge(announcementsAnswer = { answer })
        val poller = poller(jni, state, scheduler = testScheduler)
        poller.fetchOnce()

        answer = "not json at all"
        assertEquals("transport", poller.fetchOnce())

        assertEquals("a1", state.announcements.value.single().id)
    }

    @Test
    fun a_row_without_a_headline_is_dropped_and_an_unknown_severity_reads_as_info() = runTest {
        val state = WarrenAnnouncementRepository()
        val jni =
            FakeJniBridge(
                announcementsAnswer = {
                    """{"announcements":[{"id":"a1","headline":"","body":"x","level":"error"},""" +
                        """{"id":"a2","headline":"still shown","body":"x",""" +
                        """"level":"whatever"}],"fetch":"ok"}"""
                }
            )

        poller(jni, state, scheduler = testScheduler).fetchOnce()

        assertEquals(
            listOf(
                WarrenAnnouncement(
                    id = "a2",
                    headline = "still shown",
                    body = "x",
                    level = WarrenNoticeLevel.INFO,
                )
            ),
            state.announcements.value,
        )
    }

    @Test
    fun a_tunnel_between_states_defers_the_fetch_and_keeps_the_last_card() = runTest {
        val state = WarrenAnnouncementRepository()
        val jni = FakeJniBridge()
        val tunnel = FakeTunnelStateProvider(WarrenConnectedInfo.Blocking("x"))

        val fetch =
            poller(jni, state, tunnel = tunnel, scheduler = testScheduler).fetchOnce()

        assertEquals("deferred", fetch)
        assertEquals(0, jni.announcementsCalls, "the request would hang in a TUN coming up")
        assertEquals(emptyList<WarrenAnnouncement>(), state.announcements.value)
    }

    @Test
    fun nothing_is_fetched_before_the_privacy_disclosure_is_accepted() = runTest {
        val state = WarrenAnnouncementRepository()
        val jni = FakeJniBridge()
        val accepted = MutableStateFlow(false)
        val job = launch { poller(jni, state, scheduler = testScheduler).runWhile(accepted) }

        advanceTimeBy(10.minutes)
        runCurrent()
        assertEquals(0, jni.announcementsCalls)

        accepted.value = true
        runCurrent()
        assertEquals(1, jni.announcementsCalls, "accepting must fetch at once, not a poll later")

        job.cancel()
    }

    @Test
    fun the_poll_stops_with_the_window_and_resumes_with_a_fetch() = runTest {
        val state = WarrenAnnouncementRepository()
        val jni = FakeJniBridge(announcementsAnswer = { """{"announcements":[],"fetch":"ok"}""" })
        val accepted = MutableStateFlow(true)
        val job = launch { poller(jni, state, scheduler = testScheduler).runWhile(accepted) }
        runCurrent()
        assertEquals(1, jni.announcementsCalls)

        advanceTimeBy(5.minutes + 1.seconds)
        runCurrent()
        assertEquals(2, jni.announcementsCalls, "one poll every five minutes while it runs")

        job.cancel()
        runCurrent()
        advanceTimeBy(30.minutes)
        runCurrent()
        assertEquals(2, jni.announcementsCalls, "a cancelled scope must poll nothing at all")
    }

    @Test
    fun a_throwing_boundary_is_one_failed_fetch_rather_than_a_crash() = runTest {
        val state = WarrenAnnouncementRepository()
        val jni = FakeJniBridge(announcementsAnswer = { error("the library is not loaded") })

        assertEquals("transport", poller(jni, state, scheduler = testScheduler).fetchOnce())
        assertEquals(emptyList<WarrenAnnouncement>(), state.announcements.value)
    }

    @Test
    fun a_failed_code_lookup_still_shows_the_announcement() = runTest {
        val state = WarrenAnnouncementRepository()
        val jni =
            FakeJniBridge(
                announcementsAnswer = { LAUNCH_CARD },
                voucherAnswer = { """{"ok":false,"code":null}""" },
            )

        poller(jni, state, scheduler = testScheduler).fetchOnce()

        assertNull(state.announcements.value.single().voucherCode)
        assertEquals("Production is open", state.announcements.value.single().headline)
    }

    @Test
    fun the_envelope_parser_reads_the_fetch_class_and_the_rows() {
        val (announcements, fetch) =
            parseAnnouncementsEnvelope(
                """{"announcements":[{"id":"a1","headline":"one \" quote","body":"b",""" +
                    """"level":"error","cta":null,"voucher_campaign_id":null}],""" +
                    """"fetch":"rejected"}"""
            )

        assertEquals("rejected", fetch)
        assertEquals(
            listOf(
                WarrenAnnouncement(
                    id = "a1",
                    headline = """one " quote""",
                    body = "b",
                    level = WarrenNoticeLevel.ERROR,
                )
            ),
            announcements,
        )
    }

    @Test
    fun the_envelope_parser_reports_an_unreadable_answer_as_no_answer_at_all() {
        val (announcements, fetch) = parseAnnouncementsEnvelope("""{"announcements":42}""")

        assertNull(announcements, "an empty set and an unreadable answer must not look the same")
        assertEquals("transport", fetch)
    }

    @Test
    fun the_voucher_parser_yields_no_code_for_a_refusal_or_a_malformed_answer() {
        assertEquals("ABCD", parseVoucherEnvelope("""{"ok":true,"code":"ABCD"}"""))
        assertNull(parseVoucherEnvelope("""{"ok":true,"code":null}"""))
        assertNull(parseVoucherEnvelope("""{"ok":false,"code":null}"""))
        assertNull(parseVoucherEnvelope("nonsense"))
    }
}
