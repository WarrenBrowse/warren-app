package com.warrenbrowse.vpn.lib.repository

import com.warrenbrowse.vpn.fixtures.ClientRulesFixtures
import com.warrenbrowse.vpn.fixtures.ClientRulesFixtures.booleanOr
import com.warrenbrowse.vpn.fixtures.ClientRulesFixtures.cases
import com.warrenbrowse.vpn.fixtures.ClientRulesFixtures.int
import com.warrenbrowse.vpn.fixtures.ClientRulesFixtures.intOrNull
import com.warrenbrowse.vpn.fixtures.ClientRulesFixtures.ints
import com.warrenbrowse.vpn.fixtures.ClientRulesFixtures.obj
import com.warrenbrowse.vpn.fixtures.ClientRulesFixtures.string
import com.warrenbrowse.vpn.fixtures.ClientRulesFixtures.stringOrNull
import com.warrenbrowse.vpn.lib.model.forum.ForumActivityWording
import com.warrenbrowse.vpn.lib.model.forum.ForumHeaderButton
import com.warrenbrowse.vpn.lib.model.forum.UNREAD_SATURATED
import com.warrenbrowse.vpn.lib.model.forum.forumActivityWording
import com.warrenbrowse.vpn.lib.model.forum.forumHeaderButton
import com.warrenbrowse.vpn.lib.model.forum.showsForumActivity
import com.warrenbrowse.vpn.lib.model.forum.unreadForSlot
import com.warrenbrowse.vpn.lib.model.forum.unreadLabel
import kotlin.test.assertEquals
import kotlinx.serialization.json.JsonObject
import org.junit.jupiter.api.Test

/**
 * The forum badge rules replayed from `fixtures/client-rules/forum_activity.json`, the file the
 * desktop and iOS readers replay too. A rule change means changing that file and every reader in
 * the same commit; a reader is never loosened to pass.
 */
class ForumActivityFixtureTest {

    private val fixture = ClientRulesFixtures.load("forum_activity.json")

    @Test
    fun `the saturation ceiling is the one every surface counts to`() {
        assertEquals(fixture.int("unread_saturated"), UNREAD_SATURATED)
    }

    @Test
    fun `every digest is indexed the way the fixture states`() {
        for (case in fixture.cases("unread_for_slot_cases")) {
            assertEquals(
                case.int("expect"),
                unreadForSlot(case.stringOrNull("digest"), case.intOrNull("slot")),
                case.string("name"),
            )
        }
    }

    @Test
    fun `the label saturates rather than growing the badge`() {
        for (case in fixture.cases("unread_label_cases")) {
            assertEquals(case.string("expect"), unreadLabel(case.int("unread")))
        }
    }

    @Test
    fun `the header slot carries what the fixture says for every pair`() {
        for (case in fixture.cases("header_button_cases")) {
            val button =
                forumHeaderButton(
                    hasAccount = case.booleanOr("has_account", false),
                    enabled = case.booleanOr("enabled", false),
                )
            assertEquals(case.string("expect").toHeaderButton(), button, case.string("name"))
        }
    }

    @Test
    fun `activity is shown only for an account whose owner left the setting on`() {
        for (case in fixture.cases("shows_activity_cases")) {
            assertEquals(
                case.booleanOr("expect", false),
                showsForumActivity(
                    hasAccount = case.booleanOr("has_account", false),
                    enabled = case.booleanOr("enabled", false),
                ),
            )
        }
    }

    @Test
    fun `the wording of a rise follows the fixture`() {
        for (case in fixture.cases("wording_cases")) {
            val expect = case.obj("expect")
            val wording =
                when (expect.string("kind")) {
                    "single" -> ForumActivityWording.Single
                    "several" -> ForumActivityWording.Several(expect.int("count"))
                    "more_than" -> ForumActivityWording.MoreThan(expect.int("count"))
                    else -> error("unknown wording kind in ${case}")
                }
            assertEquals(wording, forumActivityWording(case.int("unread")))
        }
    }

    /**
     * The storms the monitor exists to absorb, replayed step by step: what it publishes at the end,
     * and every notification it raised on the way.
     */
    @Test
    fun `the monitor answers every storm the fixture describes`() {
        for (case in fixture.cases("monitor_cases")) {
            val notified = mutableListOf<Int>()
            var published = 0
            var indicator = false
            val monitor =
                ForumActivityMonitor(
                    object : ForumActivityMonitor.Delegate {
                        override fun notify(unread: Int) {
                            notified += unread
                        }

                        override fun showIndicator(unread: Boolean) {
                            indicator = unread
                        }

                        override fun publishUnread(count: Int) {
                            published = count
                        }
                    }
                )
            monitor.setEnabled(case.booleanOr("enabled", true))
            monitor.setSlot(case.intOrNull("slot"))
            for (step in case.cases("steps")) {
                step.applyTo(monitor)
            }

            val expect = case.obj("expect")
            val name = case.string("name")
            assertEquals(expect.int("unread"), published, name)
            assertEquals(expect.ints("notified"), notified, name)
            expect["indicator"]?.let {
                assertEquals(expect.booleanOr("indicator", false), indicator, name)
            }
        }
    }

    private fun JsonObject.applyTo(monitor: ForumActivityMonitor) {
        when {
            containsKey("digest") -> monitor.setDigest(stringOrNull("digest"))
            containsKey("observed") -> monitor.setObservedUnread(int("observed"))
            containsKey("slot") -> monitor.setSlot(intOrNull("slot"))
            containsKey("enabled") -> monitor.setEnabled(booleanOr("enabled", false))
            else -> error("unknown monitor step $this")
        }
    }

    private fun String.toHeaderButton(): ForumHeaderButton =
        when (this) {
            "activity" -> ForumHeaderButton.ACTIVITY
            "community" -> ForumHeaderButton.COMMUNITY
            "none" -> ForumHeaderButton.NONE
            else -> error("unknown header button $this")
        }
}
