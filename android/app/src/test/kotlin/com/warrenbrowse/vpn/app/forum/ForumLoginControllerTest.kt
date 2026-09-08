package com.warrenbrowse.vpn.app.forum

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

class ForumLoginControllerTest {

    private val link = ForumLoginLink(sid = "0123456789abcdef0123456789abcdef", host = "connect.warrenbrowse.com")

    // The JVM has no main looper: the scope a prompt would launch on is injected.
    private val scope = CoroutineScope(Dispatchers.Unconfined)

    @Test
    fun a_fresh_request_is_not_stale() {
        var now = 1_000_000L
        val controller = ForumLoginController(scope) { now }
        controller.request(link)
        now += (ForumLoginController.PENDING_LINK_TTL_MILLIS - 1)
        assertFalse(controller.isStale())
    }

    @Test
    fun a_request_older_than_the_server_session_ttl_is_stale() {
        // The connect login session lives 300 s. The desktop expires its
        // buffered request to match; a pending Android prompt must not offer
        // an Approve that can only ever produce a dead-session failure.
        var now = 1_000_000L
        val controller = ForumLoginController(scope) { now }
        controller.request(link)
        now += (ForumLoginController.PENDING_LINK_TTL_MILLIS + 1)
        assertTrue(controller.isStale())
    }

    @Test
    fun a_new_request_resets_the_clock() {
        var now = 1_000_000L
        val controller = ForumLoginController(scope) { now }
        controller.request(link)
        now += ForumLoginController.PENDING_LINK_TTL_MILLIS + 1
        controller.request(link)
        assertFalse(controller.isStale())
    }

    @Test
    fun clearing_leaves_nothing_pending_and_nothing_stale() {
        var now = 1_000_000L
        val controller = ForumLoginController(scope) { now }
        controller.request(link)
        controller.clear()
        // Past the TTL with nothing pending: stale must still be false, or the
        // prompt host would show an "expired" error for a link it no longer has.
        now += ForumLoginController.PENDING_LINK_TTL_MILLIS + 1
        assertNull(controller.pending.value)
        assertFalse(controller.isStale())
    }

    @Test
    fun the_prompt_state_is_the_controllers_so_a_recreated_host_sees_the_attempt_in_flight() {
        // A rotation recreates the Activity and every `remember` with it. The
        // state a host reads has to outlive that, or Approve is re-armed over
        // a signature that is still out.
        val controller = ForumLoginController(scope)
        controller.request(link)
        controller.prompt.bind(link, controller.requestToken)
        controller.prompt.begin()

        val recreated = controller.prompt
        recreated.bind(link, controller.requestToken)

        assertTrue(recreated.busy)
    }

    @Test
    fun clearing_resets_the_prompt_so_a_second_link_for_the_same_sid_shows_a_fresh_consent() {
        // The state outlives the consent on purpose (a rotation), so the
        // controller resets it when the consent ends, and every request
        // carries a new token the prompt keys on beside the sid.
        val controller = ForumLoginController(scope)
        controller.request(link)
        val first = controller.requestToken
        controller.prompt.bind(link, first)
        assertTrue(controller.prompt.markApproved(controller.prompt.begin()))

        controller.clear()
        assertFalse(controller.prompt.approved)

        controller.request(link)
        assertTrue(controller.requestToken != first, "a new request is a new token")
        controller.prompt.bind(link, controller.requestToken)
        assertFalse(controller.prompt.approved)
        assertFalse(controller.prompt.busy)
        assertEquals(link.sid, controller.prompt.sid)
    }
}
