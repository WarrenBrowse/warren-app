package com.warrenbrowse.vpn.lib.repository

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.BeforeEach
import org.junit.jupiter.api.Test

/**
 * The desktop `forum-activity-monitor.spec.ts`, rule for rule. A digest is one
 * hex character per slot; slot 2 is the one under test.
 */
class ForumActivityMonitorTest {

    private val notified = mutableListOf<Int>()
    private val indicator = mutableListOf<Boolean>()
    private val published = mutableListOf<Int>()
    private lateinit var monitor: ForumActivityMonitor

    @BeforeEach
    fun setUp() {
        monitor =
            ForumActivityMonitor(
                object : ForumActivityMonitor.Delegate {
                    override fun notify(unread: Int) {
                        notified += unread
                    }

                    override fun showIndicator(unread: Boolean) {
                        indicator += unread
                    }

                    override fun publishUnread(count: Int) {
                        published += count
                    }
                }
            )
        monitor.setEnabled(true)
        monitor.setSlot(2)
    }

    @Test
    fun announces_activity_that_arrived_while_the_app_was_running() {
        monitor.setDigest(NOTHING)
        monitor.setDigest(ONE)

        assertEquals(listOf(1), notified)
    }

    @Test
    fun stays_quiet_about_activity_that_was_already_there_at_startup() {
        // Otherwise every relaunch re-announces the same unread notifications.
        // The indicator still goes up, which is the honest way to carry a
        // state that predates this run.
        monitor.setDigest(TWO)

        assertEquals(emptyList<Int>(), notified)
        assertEquals(listOf(true), indicator)
    }

    @Test
    fun does_not_announce_the_same_count_twice() {
        monitor.setDigest(NOTHING)
        monitor.setDigest(ONE)
        monitor.setDigest(ONE)

        assertEquals(listOf(1), notified)
    }

    @Test
    fun announces_again_when_the_count_rises_further() {
        monitor.setDigest(NOTHING)
        monitor.setDigest(ONE)
        monitor.setDigest(TWO)

        assertEquals(listOf(1, 2), notified)
    }

    @Test
    fun re_announces_nothing_when_the_digest_lapses_and_comes_back() {
        // The fetch drops the document when it cannot refresh it, and the
        // count is unknown rather than zero. Treating the gap as "all read"
        // would fire a notification for what the user has already seen.
        monitor.setDigest(NOTHING)
        monitor.setDigest(TWO)
        assertEquals(listOf(2), notified)

        monitor.setDigest(null)
        monitor.setDigest(TWO)

        assertEquals(listOf(2), notified)
    }

    @Test
    fun clears_the_indicator_once_the_count_reaches_zero() {
        // Reading on the forum through any other channel advances the
        // reader's own bookmark there, so the very next digest carries zero
        // and the indicator goes out without this app being told anything.
        monitor.setDigest(ONE)
        monitor.setDigest(NOTHING)

        assertEquals(listOf(true, false), indicator)
    }

    @Test
    fun raises_the_indicator_only_when_it_actually_changes() {
        monitor.setDigest(ONE)
        monitor.setDigest(TWO)

        assertEquals(listOf(true), indicator)
    }

    @Test
    fun says_nothing_and_shows_nothing_without_the_setting() {
        monitor.setEnabled(false)
        monitor.setDigest(NOTHING)
        monitor.setDigest(ONE)

        assertEquals(emptyList<Int>(), notified)
        assertEquals(emptyList<Boolean>(), indicator)
    }

    @Test
    fun does_not_announce_what_arrived_while_the_setting_was_off() {
        monitor.setDigest(NOTHING)
        monitor.setEnabled(false)
        monitor.setDigest(TWO)
        monitor.setEnabled(true)

        assertEquals(emptyList<Int>(), notified)
        assertEquals(listOf(true), indicator)
    }

    @Test
    fun carries_no_watermark_over_from_one_forum_account_to_the_next() {
        // Slot 1 already carries 2 when we start watching it. Reading that as
        // a rise from the previous account's zero would announce, on a forum
        // login, notifications that were waiting before it.
        monitor.setDigest("020000")
        monitor.setSlot(1)

        assertEquals(emptyList<Int>(), notified)
        assertEquals(listOf(true), indicator)
    }

    @Test
    fun goes_quiet_and_dark_once_the_forum_identity_is_gone() {
        monitor.setDigest(ONE)
        assertEquals(listOf(true), indicator)

        monitor.setSlot(null)

        assertEquals(listOf(true, false), indicator)
    }

    @Test
    fun publishes_the_count_the_app_must_show() {
        // The leading zero is the boot state: the UI is told the count is
        // known and empty, rather than left guessing.
        monitor.setDigest(TWO)

        assertEquals(listOf(0, 2), published)
    }

    @Test
    fun what_the_app_observed_wins_over_the_digest_at_once() {
        // The digest is up to a minute of server refresh plus a client poll
        // behind. Reading the panel, or marking the list seen, tells the
        // truth now, and the badge must follow now.
        monitor.setDigest(TWO)
        assertEquals(listOf(true), indicator)

        monitor.setObservedUnread(0)

        assertEquals(listOf(true, false), indicator)
        assertEquals(listOf(0, 2, 0), published)
    }

    @Test
    fun the_observation_keeps_winning_while_the_digest_still_says_the_stale_thing() {
        monitor.setDigest(TWO)
        monitor.setObservedUnread(0)

        // The same document again, a minute later: still the one that
        // predates what we did, so it must not undo it.
        monitor.setDigest(TWO)

        assertEquals(listOf(true, false), indicator)
    }

    @Test
    fun the_observation_steps_aside_as_soon_as_the_digest_is_rebuilt() {
        // A changed document has seen our write, or carries something newer
        // than what we observed. Either way it is now the better source, and
        // holding the observation would freeze the badge forever.
        monitor.setDigest(TWO)
        monitor.setObservedUnread(0)
        monitor.setDigest(ONE)

        assertEquals(listOf(true, false, true), indicator)
    }

    @Test
    fun does_not_announce_what_it_observed_itself() {
        // The user is looking at the panel; a notification about it would be
        // absurd.
        monitor.setDigest(NOTHING)
        monitor.setObservedUnread(3)

        assertEquals(emptyList<Int>(), notified)
    }

    @Test
    fun does_not_re_announce_a_rise_it_already_accounted_for() {
        // Observing 3 then seeing the digest catch up to 3 is one event, not
        // two, so the notification must not fire on the digest's arrival
        // either.
        monitor.setDigest(NOTHING)
        monitor.setObservedUnread(3)
        monitor.setDigest("003000")

        assertEquals(emptyList<Int>(), notified)
    }

    private companion object {
        const val NOTHING = "000000"
        const val ONE = "001000"
        const val TWO = "002000"
    }
}
