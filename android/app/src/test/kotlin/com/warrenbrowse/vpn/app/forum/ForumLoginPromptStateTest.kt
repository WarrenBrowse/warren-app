package com.warrenbrowse.vpn.app.forum

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

class ForumLoginPromptStateTest {

    private val first =
        ForumLoginLink(sid = "0123456789abcdef0123456789abcdef", host = "connect.warrenbrowse.com")
    private val second =
        ForumLoginLink(sid = "fedcba9876543210fedcba9876543210", host = "connect.warrenbrowse.com")

    @Test
    fun a_second_link_arriving_after_a_terminal_outcome_starts_from_a_clean_prompt() {
        // The first session died on a clock-skew refusal and disarmed Approve;
        // the user fixed the clock and started again from the browser. The
        // new sid must not inherit the dead one's disarmed button and message.
        val state = ForumLoginPromptState()
        state.bind(first)
        state.settle(WarrenForumLoginOutcome.ClockSkew, message = "fix the clock")
        assertTrue(state.terminal)

        state.bind(second)

        assertFalse(state.terminal)
        assertFalse(state.busy)
        assertNull(state.failure)
        assertEquals(second.sid, state.sid)
    }

    @Test
    fun rebinding_the_same_link_keeps_the_attempt_in_flight() {
        // Recomposition binds the same link again while the signature is out;
        // that must not reset the busy marker and re-enable Approve mid-flight.
        val state = ForumLoginPromptState()
        state.bind(first)
        state.begin()

        state.bind(first)

        assertTrue(state.busy)
    }

    @Test
    fun a_non_terminal_outcome_keeps_approve_armed_with_its_message() {
        val state = ForumLoginPromptState()
        state.bind(first)
        state.begin()

        state.settle(WarrenForumLoginOutcome.Deferred("connecting"), message = "tunnel busy")

        assertFalse(state.busy)
        assertFalse(state.terminal)
        assertEquals("tunnel busy", state.failure)
    }
}
