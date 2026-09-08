package com.warrenbrowse.vpn.app.forum

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

class ForumAttachPromptStateTest {

    private val host = "connect.warrenbrowse.com"
    private val linked = ForumAttachLink(sid = "0123456789abcdef0123456789abcdef", host = host, topicId = 42L)
    private val typed = ForumAttachLink(sid = "0123456789abcdef0123456789abcdef", host = host, topicId = null)
    private val second = ForumAttachLink(sid = "fedcba9876543210fedcba9876543210", host = host, topicId = 7L)

    @Test
    fun a_link_carries_its_topic_and_needs_no_field() {
        val state = ForumAttachPromptState()
        state.bind(linked)

        assertFalse(state.needsTopic)
        assertEquals(42L, state.topicIdOrNull())
        assertTrue(state.canApprove)
    }

    @Test
    fun a_typed_code_takes_the_topic_from_the_field_and_an_empty_field_means_pre_topic() {
        // The broker's unsigned endpoints carry no topic id, so the person
        // holding the forum page supplies it; a page with no topic yet (the
        // report form) leaves the field empty.
        val state = ForumAttachPromptState()
        state.bind(typed)

        assertTrue(state.needsTopic)
        assertEquals(ForumAttachLink.PRE_TOPIC, state.topicIdOrNull())
        assertTrue(state.canApprove)

        state.updateTopicInput("198")
        assertEquals(198L, state.topicIdOrNull())

        // Anything but digits is dropped as typed: a pasted "t/198" leaves 198.
        state.updateTopicInput("t/19 8")
        assertEquals("198", state.topicInput)

        state.updateTopicInput("9007199254740993")
        assertNull(state.topicIdOrNull())
        assertFalse(state.canApprove)
    }

    @Test
    fun a_second_link_arriving_after_a_terminal_outcome_starts_from_a_clean_prompt() {
        val state = ForumAttachPromptState()
        state.bind(typed)
        state.updateTopicInput("198")
        state.settle(WarrenForumAttachOutcome.NotAuthor, message = "not yours")
        assertTrue(state.terminal)

        state.bind(second)

        assertFalse(state.terminal)
        assertFalse(state.busy)
        assertNull(state.failure)
        assertEquals("", state.topicInput)
        assertEquals(second.sid, state.sid)
    }

    @Test
    fun rebinding_the_same_link_keeps_the_attempt_in_flight() {
        val state = ForumAttachPromptState()
        state.bind(linked)
        state.begin()

        state.bind(linked)

        assertTrue(state.busy)
        assertFalse(state.canApprove)
    }

    @Test
    fun a_non_terminal_outcome_keeps_approve_armed_with_its_message() {
        val state = ForumAttachPromptState()
        state.bind(linked)
        state.begin()

        state.settle(WarrenForumAttachOutcome.ServerError, message = "later")

        assertFalse(state.busy)
        assertFalse(state.terminal)
        assertTrue(state.canApprove)
        assertEquals("later", state.failure)
    }

    @Test
    fun the_terminal_outcomes_are_the_ones_no_retry_can_change() {
        for (outcome in
            listOf(WarrenForumAttachOutcome.NotAuthor, WarrenForumAttachOutcome.Expired, WarrenForumAttachOutcome.TooLarge)) {
            assertTrue(isTerminalAttachOutcome(outcome), "$outcome")
        }
        for (outcome in
            listOf(
                WarrenForumAttachOutcome.ClockSkew,
                WarrenForumAttachOutcome.ServerError,
                WarrenForumAttachOutcome.WalletNotReady,
                WarrenForumAttachOutcome.Deferred("connecting"),
                WarrenForumAttachOutcome.Failure("transport"),
            )) {
            assertFalse(isTerminalAttachOutcome(outcome), "$outcome")
        }
    }

    @Test
    fun the_preview_belongs_to_one_link() {
        val state = ForumAttachPromptState()
        state.bind(linked)
        state.beginCollect()
        assertTrue(state.collecting)

        state.previewReady("/cache/reports/one.log")
        assertFalse(state.collecting)
        assertEquals("/cache/reports/one.log", state.previewPath)

        state.closePreview()
        assertNull(state.previewPath)

        state.previewReady("/cache/reports/two.log")
        state.bind(second)
        assertNull(state.previewPath)
    }
}
