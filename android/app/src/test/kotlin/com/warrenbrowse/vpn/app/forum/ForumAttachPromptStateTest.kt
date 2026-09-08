package com.warrenbrowse.vpn.app.forum

import com.warrenbrowse.vpn.lib.repository.CollectedReport
import java.io.File
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

class ForumAttachPromptStateTest {

    private val host = "connect.warrenbrowse.com"
    private val linked = ForumAttachLink(sid = "0123456789abcdef0123456789abcdef", host = host, topicId = 42L)
    private val second = ForumAttachLink(sid = "fedcba9876543210fedcba9876543210", host = host, topicId = 7L)

    @Test
    fun a_second_link_arriving_after_a_terminal_outcome_starts_from_a_clean_prompt() {
        val state = ForumAttachPromptState()
        state.bind(linked)
        state.settle(WarrenForumAttachOutcome.NotAuthor, message = "not yours")
        assertTrue(state.terminal)

        state.bind(second)

        assertFalse(state.terminal)
        assertFalse(state.busy)
        assertNull(state.failure)
        assertTrue(state.cancelsOnDecline)
        assertEquals(second.sid, state.sid)
    }

    @Test
    fun rebinding_the_same_link_keeps_the_attempt_in_flight() {
        // A recreated host (rotation) binds the same link again while the
        // upload is out; that must not reset the busy marker and re-arm
        // Approve mid-flight.
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
    fun leaving_a_prompt_cancels_the_session_unless_the_provider_already_closed_it() {
        // A refusal as author or a report over the cap leaves the session
        // pending on the provider, and the forum page polling it: leaving
        // the prompt must cancel it. Only a session the provider reported
        // gone is left alone: there is nothing to cancel.
        val state = ForumAttachPromptState()
        state.bind(linked)
        assertTrue(state.cancelsOnDecline)

        state.settle(WarrenForumAttachOutcome.NotAuthor, message = "not yours")
        assertTrue(state.cancelsOnDecline)

        state.settle(WarrenForumAttachOutcome.TooLarge, message = "too big")
        assertTrue(state.cancelsOnDecline)

        state.settle(WarrenForumAttachOutcome.ServerError, message = "later")
        assertTrue(state.cancelsOnDecline)

        state.settle(WarrenForumAttachOutcome.Expired, message = "gone")
        assertFalse(state.cancelsOnDecline)
    }

    @Test
    fun an_attached_outcome_is_held_for_the_host_until_the_next_link() {
        // The upload runs on the controller's scope, so a host recreated by
        // a rotation mid-flight still learns the outcome from the state.
        val state = ForumAttachPromptState()
        state.bind(linked)
        state.begin()

        state.markAttached()

        assertTrue(state.attached)
        assertFalse(state.busy)

        state.bind(second)
        assertFalse(state.attached)
    }

    @Test
    fun the_preview_belongs_to_one_link() {
        val state = ForumAttachPromptState()
        state.bind(linked)
        state.beginCollect()
        assertTrue(state.collecting)

        val report = CollectedReport(File("/cache/reports/one.log"), bytes = 1)
        state.previewReady(report)
        assertFalse(state.collecting)
        assertEquals(report, state.preview)
        assertEquals("/cache/reports/one.log", state.previewPath)

        state.closePreview()
        assertNull(state.previewPath)
        assertEquals(report, state.preview, "the file is kept for the approval, only the screen closes")

        state.bind(second)
        assertNull(state.preview)
    }
}
