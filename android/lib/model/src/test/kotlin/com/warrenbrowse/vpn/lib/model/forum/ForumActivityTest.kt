package com.warrenbrowse.vpn.lib.model.forum

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

class ForumActivityTest {

    @Test
    fun the_slot_is_indexed_into_the_digest_one_hex_character_per_slot() {
        // The desktop `unreadForSlot`: slot 2 of "002000" carries 2.
        assertEquals(2, unreadForSlot("002000", 2))
        assertEquals(0, unreadForSlot("002000", 0))
        assertEquals(15, unreadForSlot("00f", 2))
        assertEquals(10, unreadForSlot("a", 0))
    }

    @Test
    fun nothing_to_show_reads_as_zero() {
        // No fresh document, no slot yet, a slot past what the server has
        // published (an account that registered since the last rebuild), or
        // a character that is not hex: all zero, never an error.
        assertEquals(0, unreadForSlot(null, 2))
        assertEquals(0, unreadForSlot("", 0))
        assertEquals(0, unreadForSlot("002000", null))
        assertEquals(0, unreadForSlot("002000", 6))
        assertEquals(0, unreadForSlot("002000", -1))
        assertEquals(0, unreadForSlot("00z000", 2))
    }

    @Test
    fun the_count_saturates_at_the_digest_ceiling() {
        assertEquals(15, UNREAD_SATURATED)
        assertEquals("15+", unreadLabel(15))
        assertEquals("15+", unreadLabel(40))
        assertEquals("3", unreadLabel(3))
    }

    @Test
    fun the_header_slot_carries_the_bell_the_lifebuoy_or_nothing() {
        // The desktop `forumHeaderButton` truth table: the setting governs the
        // whole slot, lifebuoy included, and an accountless wallet gets the
        // way into the forum rather than an inert bell.
        assertEquals(ForumHeaderButton.ACTIVITY, forumHeaderButton(hasAccount = true, enabled = true))
        assertEquals(ForumHeaderButton.COMMUNITY, forumHeaderButton(hasAccount = false, enabled = true))
        assertEquals(ForumHeaderButton.NONE, forumHeaderButton(hasAccount = true, enabled = false))
        assertEquals(ForumHeaderButton.NONE, forumHeaderButton(hasAccount = false, enabled = false))
    }

    @Test
    fun activity_shows_only_for_an_account_with_the_setting_on() {
        assertTrue(showsForumActivity(hasAccount = true, enabled = true))
        assertFalse(showsForumActivity(hasAccount = false, enabled = true))
        assertFalse(showsForumActivity(hasAccount = true, enabled = false))
    }

    @Test
    fun the_rise_wording_counts_one_several_and_the_saturated_ceiling_differently() {
        assertEquals(ForumActivityWording.Single, forumActivityWording(1))
        assertEquals(ForumActivityWording.Several(2), forumActivityWording(2))
        // "15" would be a number the user can check and find wrong.
        assertEquals(ForumActivityWording.MoreThan(14), forumActivityWording(15))
    }

    @Test
    fun an_age_reads_relative_for_a_week_then_as_a_date() {
        val now = 1_800_000_000L
        assertTrue(forumNotificationAgeIsRelative(now - 2 * 3600, now))
        assertTrue(forumNotificationAgeIsRelative(now - 6 * 86_400, now))
        assertFalse(forumNotificationAgeIsRelative(now - 40 * 86_400, now))
    }

    @Test
    fun a_notification_kind_maps_from_its_token_and_unknown_tokens_stay_generic() {
        assertEquals(ForumNotificationKind.REPLIED, ForumNotificationKind.fromToken("replied"))
        assertEquals(ForumNotificationKind.PRIVATE_MESSAGE, ForumNotificationKind.fromToken("private_message"))
        assertEquals(ForumNotificationKind.OTHER, ForumNotificationKind.fromToken("chat_mention"))
        assertEquals(ForumNotificationKind.OTHER, ForumNotificationKind.fromToken(null))
    }
}
