package com.warrenbrowse.vpn.app.forum

import com.warrenbrowse.vpn.fixtures.ClientRulesFixtures
import com.warrenbrowse.vpn.fixtures.ClientRulesFixtures.cases
import com.warrenbrowse.vpn.fixtures.ClientRulesFixtures.string
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.long
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

/**
 * Which screen an approved login leads to, replayed from the `login.completion`
 * table of `fixtures/client-rules/forum_outcomes.json`: the approach, and the
 * FFI envelope of the answer the case names, decoded as the use case decodes it.
 */
class ForumLoginCompletionTest {

    private val login = ClientRulesFixtures.load("forum_outcomes.json")["login"]!!.jsonObject
    private val completion = login["completion"]!!.jsonObject

    @Test
    fun every_approach_leads_to_the_screen_and_the_handoff_the_fixture_names() {
        val answers = login.cases("cases").associateBy { it.string("name") }
        val approaches = completion["approaches"]!!.jsonArray.map { it.jsonPrimitive.content }
        assertEquals(ForumLoginApproach.entries.map { it.token }.sorted(), approaches.sorted())
        val cases = completion.cases("cases").filterNot(ClientRulesFixtures::skippedOnAndroid)
        assertTrue(cases.size >= 9, "only ${cases.size} completion cases reached this reader")
        for (case in cases) {
            val name = case.string("name")
            val answer = answers[case.string("answer")] ?: error("$name: no login case ${case.string("answer")}")
            val approach = ForumLoginApproach.entries.single { it.token == case.string("approach") }
            val outcome = parseForumLoginOutcome(answer.string("envelope"))
            check(outcome is WarrenForumLoginOutcome.Approved) { "$name: the answer is an approval" }
            val plan = forumCompletionPlan(approach, outcome.completion)
            val expect = case["expect"]!!.jsonObject
            assertEquals(expect.string("screen"), plan.screen.token, "$name: screen")
            assertEquals(expect.string("handoff"), plan.handoff.token, "$name: handoff")
        }
    }

    @Test
    fun the_code_lives_no_longer_than_the_session() {
        assertEquals(
            completion["code_lifetime_secs"]!!.jsonPrimitive.long * 1000,
            FORUM_LOGIN_CODE_LIFETIME_MILLIS,
        )
    }
}
