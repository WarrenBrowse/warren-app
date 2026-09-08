package com.warrenbrowse.vpn.app.forum

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

class ForumAttachControllerTest {

    private val link =
        ForumAttachLink(sid = "0123456789abcdef0123456789abcdef", host = "connect.warrenbrowse.com", topicId = 0L)

    // The JVM has no main looper: the scope a prompt would launch on is injected.
    private val scope = CoroutineScope(Dispatchers.Unconfined)

    @Test
    fun clearing_resets_the_prompt_so_a_second_link_for_the_same_sid_shows_a_fresh_consent() {
        // A pre-topic session stays alive as received and a pending topic
        // hands the same sid back, so re-tapping the page's button after a
        // completed attempt binds the same sid again. Without the reset and
        // the per-request token the attached flag survived, and the host
        // toasted, backgrounded the app and cleared the consent with no
        // upload at all.
        val controller = ForumAttachController(scope)
        controller.request(link)
        val first = controller.requestToken
        controller.prompt.bind(link, first)
        assertTrue(controller.prompt.markAttached(controller.prompt.begin()))

        controller.clear()
        assertFalse(controller.prompt.attached)
        assertNull(controller.prompt.sid)

        controller.request(link)
        assertTrue(controller.requestToken != first, "a new request is a new token")
        controller.prompt.bind(link, controller.requestToken)
        assertFalse(controller.prompt.attached)
        assertFalse(controller.prompt.busy)
        assertTrue(controller.prompt.canApprove)
    }

    @Test
    fun a_request_older_than_the_attach_session_ttl_is_stale() {
        var now = 1_000_000L
        val controller = ForumAttachController(scope) { now }
        controller.request(link)
        now += ForumAttachController.PENDING_LINK_TTL_MILLIS + 1
        assertTrue(controller.isStale())
    }
}
