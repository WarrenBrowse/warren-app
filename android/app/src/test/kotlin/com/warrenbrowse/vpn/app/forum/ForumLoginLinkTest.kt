package com.warrenbrowse.vpn.app.forum

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Test

class ForumLoginLinkTest {

    private val sid = "a".repeat(32)
    private val good = "warren://forum-login?sid=$sid&host=connect.warrenbrowse.com"

    // The scheme is pinned so these tests pass under every product flavor:
    // the default argument resolves to the flavor's own scheme (warren-beta on
    // beta), which is exercised separately below.
    private fun parse(url: String?) = parseForumLoginLink(url, expectedScheme = "warren")

    @Test
    fun accepts_a_well_formed_allowlisted_link() {
        assertEquals(ForumLoginLink(sid, "connect.warrenbrowse.com"), parse(good))
    }

    @Test
    fun rejects_a_non_allowlisted_host_so_a_hostile_link_cannot_redirect_a_signed_request() {
        assertNull(parse("warren://forum-login?sid=$sid&host=evil.example.com"))
    }

    @Test
    fun rejects_a_malformed_sid() {
        assertNull(parse("warren://forum-login?sid=NOTHEX&host=connect.warrenbrowse.com"))
        val upper = "A".repeat(32)
        assertNull(parse("warren://forum-login?sid=$upper&host=connect.warrenbrowse.com"))
        val tooShort = "a".repeat(31)
        assertNull(parse("warren://forum-login?sid=$tooShort&host=connect.warrenbrowse.com"))
    }

    @Test
    fun rejects_the_wrong_scheme_or_action() {
        assertNull(parse("https://forum-login?sid=$sid&host=connect.warrenbrowse.com"))
        assertNull(parse("warren://something-else?sid=$sid&host=connect.warrenbrowse.com"))
    }

    @Test
    fun rejects_a_missing_param_or_non_url_without_throwing() {
        assertNull(parse("warren://forum-login?sid=$sid"))
        assertNull(parse("warren://forum-login?host=connect.warrenbrowse.com"))
        assertNull(parse("not a url"))
        assertNull(parse(null))
    }

    @Test
    fun `scheme must match the flavor's registered deep-link scheme`() {
        val sid = "0123456789abcdef0123456789abcdef"
        val betaLink = "warren-beta://forum-login?sid=$sid&host=connect.warrenbrowse.com"
        assertEquals(
            ForumLoginLink(sid, "connect.warrenbrowse.com"),
            parseForumLoginLink(betaLink, expectedScheme = "warren-beta"),
        )
        // A prod link must not be answered by a beta build, nor vice versa.
        assertNull(parseForumLoginLink(betaLink, expectedScheme = "warren"))
        assertNull(
            parseForumLoginLink(
                "warren://forum-login?sid=$sid&host=connect.warrenbrowse.com",
                expectedScheme = "warren-beta",
            ),
        )
    }

    @Test
    fun the_qr_link_is_marked_cross_device_so_the_prompt_can_say_so() {
        // A relayed sign-in and a legitimate cross-device one are identical on
        // the wire. This flag is what lets the person approving tell them
        // apart, so it has to survive the parser.
        assertEquals(true, parse("$good&xd=1")?.crossDevice)
    }

    @Test
    fun anything_but_xd_equals_one_is_the_same_device_button() {
        // An older provider sends no flag at all and must degrade to the
        // ordinary prompt, never to a warning nobody can act on.
        for (suffix in listOf("", "&xd=0", "&xd=true", "&xd=")) {
            assertEquals(false, parse("$good$suffix")?.crossDevice, "suffix: $suffix")
        }
    }

    @Test
    fun a_rejected_link_names_its_class_never_its_values() {
        // The class is the fact a report needs to show a broker/app drift
        // (a prod scheme reaching a beta install); the sid must never be in it.
        val sid = "0123456789abcdef0123456789abcdef"
        fun reason(url: String?) =
            (classifyForumLoginLink(url, expectedScheme = "warren-beta") as ForumLinkVerdict.Rejected).reason
        assertEquals("wrong-scheme:warren", reason("warren://forum-login?sid=$sid&host=connect.warrenbrowse.com"))
        assertEquals("wrong-action", reason("warren-beta://attach-logs?sid=$sid&host=connect.warrenbrowse.com"))
        assertEquals("missing-sid", reason("warren-beta://forum-login?host=connect.warrenbrowse.com"))
        assertEquals("bad-sid-shape", reason("warren-beta://forum-login?sid=ABC&host=connect.warrenbrowse.com"))
        assertEquals("host-not-allowlisted", reason("warren-beta://forum-login?sid=$sid&host=evil.example.com"))
        assertEquals("no-data", reason(null))
        assertEquals("not-a-uri", reason("::not a uri::"))
        val accepted =
            classifyForumLoginLink(
                "warren-beta://forum-login?sid=$sid&host=connect.warrenbrowse.com",
                expectedScheme = "warren-beta",
            ) as ForumLinkVerdict.Accepted
        assertEquals(ForumLoginLink(sid, "connect.warrenbrowse.com"), accepted.link)
    }

    @Test
    fun a_typed_code_stands_for_the_same_device_link_on_the_allowlisted_host() {
        val sid = "0123456789abcdef0123456789abcdef"
        assertEquals(ForumLoginLink(sid, "connect.warrenbrowse.com", crossDevice = false), forumLoginLinkFromCode(sid))
    }
}
