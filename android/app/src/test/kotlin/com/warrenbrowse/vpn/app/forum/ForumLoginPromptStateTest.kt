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
        state.bind(first, token = 1L)
        state.settle(state.begin(), WarrenForumLoginOutcome.ClockSkew, message = "fix the clock")
        assertTrue(state.terminal)

        state.bind(second, token = 2L)

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
        state.bind(first, token = 1L)
        state.begin()

        state.bind(first, token = 1L)

        assertTrue(state.busy)
    }

    @Test
    fun a_non_terminal_outcome_keeps_approve_armed_with_its_message() {
        val state = ForumLoginPromptState()
        state.bind(first, token = 1L)
        val attempt = state.begin()

        state.settle(attempt, WarrenForumLoginOutcome.Deferred("connecting"), message = "tunnel busy")

        assertFalse(state.busy)
        assertFalse(state.terminal)
        assertEquals("tunnel busy", state.failure)
    }

    @Test
    fun an_approval_is_held_for_the_host_until_the_next_link() {
        // The signature runs on the controller's scope, so a host recreated by
        // a rotation mid-flight still learns the approval from the state
        // rather than re-arming Approve over a login that already happened.
        val state = ForumLoginPromptState()
        state.bind(first, token = 1L)
        val attempt = state.begin()

        assertTrue(state.markApproved(attempt))

        assertTrue(state.approved)
        assertFalse(state.busy)

        state.bind(second, token = 2L)
        assertFalse(state.approved)
    }

    @Test
    fun a_new_request_for_the_same_sid_starts_a_fresh_consent() {
        val state = ForumLoginPromptState()
        state.bind(first, token = 1L)
        assertTrue(state.markApproved(state.begin()))

        state.bind(first, token = 2L)

        assertFalse(state.approved)
        assertFalse(state.busy)
        assertNull(state.failure)
    }

    @Test
    fun a_result_for_a_superseded_attempt_is_dropped_never_applied_to_the_current_link() {
        val state = ForumLoginPromptState()
        state.bind(first, token = 1L)
        val attempt = state.begin()

        state.bind(second, token = 2L)

        assertFalse(state.markApproved(attempt))
        assertFalse(state.approved)
        assertFalse(state.settle(attempt, WarrenForumLoginOutcome.Expired, message = "gone"))
        assertNull(state.failure)
        assertFalse(state.terminal)
    }

    private val code = "042917"
    private val handoff =
        "https://connect.warrenbrowse.com/handoff#sid=0123456789abcdef0123456789abcdef&code=042917"

    @Test
    fun a_same_device_approval_opens_the_handoff_once_and_keeps_the_code_behind_a_reveal() {
        val state = ForumLoginPromptState()
        state.bind(first, token = 1L)
        val attempt = state.begin()

        assertTrue(state.complete(attempt, first, ForumLoginCompletion(code, handoff), nowMillis = 1_000L))

        assertFalse(state.approved)
        assertFalse(state.busy)
        val view = state.completion!!
        assertEquals(ForumCompletionScreen.FINISHING_IN_BROWSER, view.screen)
        assertFalse(state.codeRevealed)
        assertEquals(handoff, state.takeHandoffToOpen(1_000L))
        assertNull(state.takeHandoffToOpen(1_000L))
        assertNull(state.takeFinishUrl(1_000L))
        state.revealCode()
        assertTrue(state.codeRevealed)
    }

    @Test
    fun a_typed_code_keeps_its_handoff_for_the_button_only() {
        val typed = forumLoginLinkFromCode(first.sid)
        val state = ForumLoginPromptState()
        state.bind(typed, token = 1L)

        state.complete(state.begin(), typed, ForumLoginCompletion(code, handoff), nowMillis = 1_000L)

        assertEquals(ForumCompletionScreen.SHOW_CODE, state.completion!!.screen)
        assertTrue(state.codeRevealed)
        assertTrue(state.completion!!.finishInBrowser)
        assertNull(state.takeHandoffToOpen(1_000L))
        assertEquals(handoff, state.takeFinishUrl(1_000L))
        assertNull(state.takeFinishUrl(1_000L))
    }

    @Test
    fun a_qr_approval_never_hands_a_handoff_over() {
        val qr = first.copy(crossDevice = true)
        val state = ForumLoginPromptState()
        state.bind(qr, token = 1L)

        state.complete(state.begin(), qr, ForumLoginCompletion(code, handoff), nowMillis = 1_000L)

        assertEquals(ForumCompletionScreen.SHOW_CODE, state.completion!!.screen)
        assertFalse(state.completion!!.finishInBrowser)
        assertNull(state.takeHandoffToOpen(1_000L))
        assertNull(state.takeFinishUrl(1_000L))
    }

    @Test
    fun an_answer_without_a_completion_is_the_approval_the_browser_finishes() {
        val state = ForumLoginPromptState()
        state.bind(first, token = 1L)

        state.complete(state.begin(), first, completion = null, nowMillis = 1_000L)

        assertTrue(state.approved)
        assertNull(state.completion)
    }

    @Test
    fun the_code_goes_with_its_session_and_with_the_next_link() {
        val state = ForumLoginPromptState()
        state.bind(first, token = 1L)
        state.complete(state.begin(), first, ForumLoginCompletion(code, null), nowMillis = 1_000L)

        assertFalse(state.codeExpired(1_000L + FORUM_LOGIN_CODE_LIFETIME_MILLIS - 1))
        assertTrue(state.codeExpired(1_000L + FORUM_LOGIN_CODE_LIFETIME_MILLIS))

        state.bind(second, token = 2L)
        assertNull(state.completion)
        assertFalse(state.codeExpired(Long.MAX_VALUE))
    }

    @Test
    fun a_completion_outliving_its_session_hands_no_handoff_over() {
        // A result that lands while no host is on screen stays on the
        // controller; the next host must not open a handoff of a dead session.
        val late = 1_000L + FORUM_LOGIN_CODE_LIFETIME_MILLIS
        val link = first
        val state = ForumLoginPromptState()
        state.bind(link, token = 1L)
        state.complete(state.begin(), link, ForumLoginCompletion(code, handoff), nowMillis = 1_000L)
        assertNull(state.takeHandoffToOpen(late))

        val typed = forumLoginLinkFromCode(first.sid)
        state.bind(typed, token = 2L)
        state.complete(state.begin(), typed, ForumLoginCompletion(code, handoff), nowMillis = 1_000L)
        assertNull(state.takeFinishUrl(late))
    }

    @Test
    fun a_superseded_attempt_shows_no_code() {
        val state = ForumLoginPromptState()
        state.bind(first, token = 1L)
        val stale = state.begin()
        state.bind(second, token = 2L)

        assertFalse(state.complete(stale, first, ForumLoginCompletion(code, handoff), nowMillis = 1_000L))
        assertNull(state.completion)
        assertNull(state.takeHandoffToOpen(1_000L))
    }
}
