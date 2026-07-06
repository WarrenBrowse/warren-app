package com.warrenbrowse.vpn.app.forum

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Test

class ForumLoginLinkTest {

    private val sid = "a".repeat(32)
    private val good = "warren://forum-login?sid=$sid&host=connect.warrenbrowse.com"

    @Test
    fun accepts_a_well_formed_allowlisted_link() {
        assertEquals(ForumLoginLink(sid, "connect.warrenbrowse.com"), parseForumLoginLink(good))
    }

    @Test
    fun rejects_a_non_allowlisted_host_so_a_hostile_link_cannot_redirect_a_signed_request() {
        assertNull(parseForumLoginLink("warren://forum-login?sid=$sid&host=evil.example.com"))
    }

    @Test
    fun rejects_a_malformed_sid() {
        assertNull(parseForumLoginLink("warren://forum-login?sid=NOTHEX&host=connect.warrenbrowse.com"))
        val upper = "A".repeat(32)
        assertNull(parseForumLoginLink("warren://forum-login?sid=$upper&host=connect.warrenbrowse.com"))
        val tooShort = "a".repeat(31)
        assertNull(parseForumLoginLink("warren://forum-login?sid=$tooShort&host=connect.warrenbrowse.com"))
    }

    @Test
    fun rejects_the_wrong_scheme_or_action() {
        assertNull(parseForumLoginLink("https://forum-login?sid=$sid&host=connect.warrenbrowse.com"))
        assertNull(parseForumLoginLink("warren://something-else?sid=$sid&host=connect.warrenbrowse.com"))
    }

    @Test
    fun rejects_a_missing_param_or_non_url_without_throwing() {
        assertNull(parseForumLoginLink("warren://forum-login?sid=$sid"))
        assertNull(parseForumLoginLink("warren://forum-login?host=connect.warrenbrowse.com"))
        assertNull(parseForumLoginLink("not a url"))
        assertNull(parseForumLoginLink(null))
    }
}
