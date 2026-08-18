package com.warrenbrowse.vpn.app.forum

import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

class ForumLoginControllerTest {

    private val link = ForumLoginLink(sid = "0123456789abcdef0123456789abcdef", host = "connect.warrenbrowse.com")

    @Test
    fun a_fresh_request_is_not_stale() {
        var now = 1_000_000L
        val controller = ForumLoginController { now }
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
        val controller = ForumLoginController { now }
        controller.request(link)
        now += (ForumLoginController.PENDING_LINK_TTL_MILLIS + 1)
        assertTrue(controller.isStale())
    }

    @Test
    fun a_new_request_resets_the_clock() {
        var now = 1_000_000L
        val controller = ForumLoginController { now }
        controller.request(link)
        now += ForumLoginController.PENDING_LINK_TTL_MILLIS + 1
        controller.request(link)
        assertFalse(controller.isStale())
    }

    @Test
    fun clearing_leaves_nothing_pending_and_nothing_stale() {
        val controller = ForumLoginController { 0L }
        controller.request(link)
        controller.clear()
        assertNull(controller.pending.value)
        assertFalse(controller.isStale())
    }
}
