package com.warrenbrowse.vpn.app.forum

import com.warrenbrowse.vpn.fixtures.ClientRulesFixtures
import com.warrenbrowse.vpn.fixtures.ClientRulesFixtures.cases
import com.warrenbrowse.vpn.fixtures.ClientRulesFixtures.string
import com.warrenbrowse.vpn.fixtures.ClientRulesFixtures.stringOrNull
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.long
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

class ForumAttachLinkTest {

    private val sid = "0123456789abcdef0123456789abcdef"

    // The cross-platform fixture (fixtures/client-rules/README.md), replayed
    // here with the full rejection-class vocabulary, and by the desktop suite
    // on its side of the same file.
    private val fixture = ClientRulesFixtures.load("forum_link.json")

    @Test
    fun the_shared_attach_fixture_replays_case_for_case() {
        val cases = fixture.cases("attach_cases").filterNot(ClientRulesFixtures::skippedOnAndroid)
        assertTrue(cases.size >= 10, "only ${cases.size} attach cases reached this reader")
        for (case in cases) {
            val name = case.string("name")
            val verdict =
                classifyForumAttachLink(case.stringOrNull("url"), expectedScheme = case.string("expected_scheme"))
            val expect = case["expect"]!!.jsonObject
            val accepted = expect["accepted"]?.jsonObject
            val expected =
                if (accepted != null) {
                    ForumAttachVerdict.Accepted(
                        ForumAttachLink(
                            accepted.string("sid"),
                            accepted.string("host"),
                            topicId = accepted["topic_id"]!!.jsonPrimitive.long,
                        )
                    )
                } else {
                    ForumAttachVerdict.Rejected(expect.string("rejected"))
                }
            assertEquals(expected, verdict, name)
        }
    }

    @Test
    fun the_attach_lifetime_is_the_fixtures() {
        // The pending link outlives the login's by an order of magnitude: the
        // attach session lives 1800 s on the broker, and a prompt that expired
        // its link at the login's 300 s would refuse a page still waiting.
        val attachTtlSecs = fixture["pending_ttl_secs"]!!.jsonObject["attach"]!!.jsonPrimitive.long
        assertEquals(attachTtlSecs * 1000, ForumAttachController.PENDING_LINK_TTL_MILLIS)
    }

    @Test
    fun a_deep_link_is_routed_by_its_action_before_either_parser_sees_it() {
        // One intent filter, two flows: the action is what tells them apart,
        // and a link the login parser would refuse as `wrong-action` must
        // reach the attach parser rather than the log.
        assertEquals("attach-logs", forumDeepLinkAction("warren://attach-logs?sid=$sid&topic=42&host=x"))
        assertEquals("forum-login", forumDeepLinkAction("warren://forum-login?sid=$sid&host=x"))
        assertNull(forumDeepLinkAction("::not a uri::"))
        assertNull(forumDeepLinkAction(null))
    }

    @Test
    fun a_typed_code_stands_for_the_attach_session_the_meta_named() {
        // The attach page prints its session id like the sign-in code; the
        // broker's meta names the topic, so the link the code stands for
        // carries it, and a pre-topic session carries the pre-topic id.
        assertEquals(
            ForumAttachLink(sid, "connect.warrenbrowse.com", topicId = 199L),
            forumAttachLinkFromCode(sid, topicId = 199L),
        )
        assertEquals(
            ForumAttachLink(sid, "connect.warrenbrowse.com", topicId = ForumAttachLink.PRE_TOPIC),
            forumAttachLinkFromCode(sid, topicId = ForumAttachLink.PRE_TOPIC),
        )
    }
}
