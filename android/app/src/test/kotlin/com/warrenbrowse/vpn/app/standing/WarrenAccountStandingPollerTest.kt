package com.warrenbrowse.vpn.app.standing

import com.warrenbrowse.vpn.app.forum.FakeWalletRepository
import com.warrenbrowse.vpn.lib.model.AbuseCategory
import com.warrenbrowse.vpn.lib.model.AccountBan
import com.warrenbrowse.vpn.lib.model.StrikeNotice
import com.warrenbrowse.vpn.lib.model.wallet.WalletState
import com.warrenbrowse.vpn.lib.repository.AccountStrikeAlerts
import com.warrenbrowse.vpn.lib.repository.WarrenAccountStandingRepository
import com.warrenbrowse.vpn.lib.repository.WarrenStandingBridge
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue
import kotlin.time.Duration.Companion.minutes
import kotlin.time.TestTimeSource
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.jupiter.api.Test

private const val STRIKE_1 =
    """{"day_unix_secs":1790208000,"category":"copyright","exit_country":"FI","port":51413,"case_reference":"PF-1"}"""

private const val STANDING_ONE_STRIKE =
    """{"ok":true,"reported":true,"standing":{"strikes":[$STRIKE_1],"threshold":3,"window_days":90,"ban":null},""" +
        """"new_strikes":[{"strike":$STRIKE_1,"ordinal":1,"threshold":3}]}"""

private const val STANDING_SAME_AGAIN =
    """{"ok":true,"reported":true,"standing":{"strikes":[$STRIKE_1],"threshold":3,"window_days":90,"ban":null},""" +
        """"new_strikes":[]}"""

private const val STANDING_BANNED =
    """{"ok":true,"reported":true,"standing":{"strikes":[],"threshold":3,"window_days":90,""" +
        """"ban":{"reason":"port_forwarding_abuse","banned_at_unix_secs":1790000000,""" +
        """"lapses_at_unix_secs":1821536000,"in_force":true,"block_reason":"[BANNED_PORT_FORWARDING] x"}},""" +
        """"new_strikes":[]}"""

private const val NOT_REPORTED = """{"ok":true,"reported":false,"standing":null,"new_strikes":[]}"""

private const val FAILED = """{"ok":false,"reported":true,"standing":null,"new_strikes":[]}"""

private class FakeStandingBridge(vararg answers: String) : WarrenStandingBridge {
    private val queue = ArrayDeque(answers.toList())
    val phrases = mutableListOf<String>()
    var forgets = 0

    override fun accountStanding(mnemonic: String): String {
        phrases += mnemonic
        return queue.removeFirstOrNull() ?: error("no answer queued")
    }

    override fun forgetAccountStanding() {
        forgets++
    }
}

private class RecordingAlerts : AccountStrikeAlerts {
    val announced = mutableListOf<StrikeNotice>()
    var clears = 0

    override fun announce(notice: StrikeNotice) {
        announced += notice
    }

    override fun clear() {
        clears++
    }
}

@OptIn(ExperimentalCoroutinesApi::class)
class WarrenAccountStandingPollerTest {

    @Test
    fun `the envelope reads the strikes, the ban and the strikes to warn about`() {
        val banned = assertNotNull(parseStandingEnvelope(STANDING_BANNED))
        assertEquals(
            AccountBan(portForwarding = true, lapsesAtUnixSecs = 1_821_536_000, inForce = true),
            banned.standing?.ban,
        )

        val poll = assertNotNull(parseStandingEnvelope(STANDING_ONE_STRIKE))
        val strike = poll.standing!!.strikes.single()
        assertEquals(51413, strike.port)
        assertEquals("PF-1", strike.caseReference)
        assertEquals(AbuseCategory.Copyright, strike.category)
        assertEquals(listOf(StrikeNotice(strike, 1, 3)), poll.newStrikes)
    }

    @Test
    fun `an unreadable envelope is no poll at all`() {
        assertNull(parseStandingEnvelope("not json"))
        assertNull(parseStandingEnvelope("[]"))
    }

    @Test
    fun `each new strike raises one notification, and the same standing again raises none`() =
        runTest {
            val state = WarrenAccountStandingRepository()
            val alerts = RecordingAlerts()
            val poller =
                WarrenAccountStandingPoller(
                    FakeStandingBridge(STANDING_ONE_STRIKE, STANDING_SAME_AGAIN),
                    state,
                    alerts,
                    FakeWalletRepository(),
                    UnconfinedTestDispatcher(testScheduler),
                )

            assertTrue(poller.pollOnce())
            assertTrue(poller.pollOnce())

            assertEquals(listOf("PF-1"), alerts.announced.map { it.strike.caseReference })
            assertEquals(1, state.standing.value?.strikes?.size)
        }

    @Test
    fun `a failed poll keeps the standing already shown`() = runTest {
        val state = WarrenAccountStandingRepository()
        val poller =
            WarrenAccountStandingPoller(
                FakeStandingBridge(STANDING_BANNED, FAILED),
                state,
                RecordingAlerts(),
                FakeWalletRepository(),
                UnconfinedTestDispatcher(testScheduler),
            )
        poller.pollOnce()

        assertFalse(poller.pollOnce())

        assertEquals(1_821_536_000L, state.standing.value?.ban?.lapsesAtUnixSecs)
    }

    @Test
    fun `an api that does not report the standing shows nothing and counts as answered`() =
        runTest {
            val state = WarrenAccountStandingRepository()
            val poller =
                WarrenAccountStandingPoller(
                    FakeStandingBridge(NOT_REPORTED),
                    state,
                    RecordingAlerts(),
                    FakeWalletRepository(),
                    UnconfinedTestDispatcher(testScheduler),
                )

            assertTrue(poller.pollOnce())

            assertNull(state.standing.value)
        }

    @Test
    fun `no wallet, no poll`() = runTest {
        val bridge = FakeStandingBridge()
        val poller =
            WarrenAccountStandingPoller(
                bridge,
                WarrenAccountStandingRepository(),
                RecordingAlerts(),
                FakeWalletRepository(WalletState.Absent),
                UnconfinedTestDispatcher(testScheduler),
            )

        assertFalse(poller.pollOnce())
        assertTrue(bridge.phrases.isEmpty())
    }

    @Test
    fun `a poll younger than the cadence is not repeated`() = runTest {
        val bridge = FakeStandingBridge(STANDING_SAME_AGAIN, STANDING_SAME_AGAIN)
        val clock = TestTimeSource()
        val poller =
            WarrenAccountStandingPoller(
                bridge,
                WarrenAccountStandingRepository(),
                RecordingAlerts(),
                FakeWalletRepository(),
                UnconfinedTestDispatcher(testScheduler),
                clock,
            )

        poller.pollIfDue()
        clock += 9.minutes
        poller.pollIfDue()
        assertEquals(1, bridge.phrases.size)

        clock += 1.minutes
        poller.pollIfDue()
        assertEquals(2, bridge.phrases.size)
    }

    @Test
    fun `a wallet that leaves takes its standing and its warnings with it`() = runTest {
        val state = WarrenAccountStandingRepository()
        val alerts = RecordingAlerts()
        val bridge = FakeStandingBridge(STANDING_ONE_STRIKE)
        val wallet = FakeWalletRepository()
        val poller =
            WarrenAccountStandingPoller(
                bridge,
                state,
                alerts,
                wallet,
                UnconfinedTestDispatcher(testScheduler),
            )
        val job = launch { poller.runWhile(MutableStateFlow(true)) }
        runCurrent()
        assertNotNull(state.standing.value)

        wallet.stateFlow.value = WalletState.Absent
        runCurrent()

        assertNull(state.standing.value)
        assertEquals(1, alerts.clears)
        assertEquals(1, bridge.forgets)
        job.cancel()
    }
}
