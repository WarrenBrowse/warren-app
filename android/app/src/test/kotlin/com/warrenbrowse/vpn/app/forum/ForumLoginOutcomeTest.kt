package com.warrenbrowse.vpn.app.forum

import com.warrenbrowse.vpn.fixtures.ClientRulesFixtures
import com.warrenbrowse.vpn.fixtures.ClientRulesFixtures.cases
import com.warrenbrowse.vpn.fixtures.ClientRulesFixtures.string
import com.warrenbrowse.vpn.fixtures.ClientRulesFixtures.stringOrNull
import com.warrenbrowse.vpn.lib.model.forum.ForumIdentity
import kotlinx.serialization.json.intOrNull
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

class ForumLoginOutcomeTest {

    @Test
    fun ok_maps_to_approved() {
        assertEquals(
            WarrenForumLoginOutcome.Approved(identity = null),
            parseForumLoginOutcome("""{"ok":true}"""),
        )
    }

    @Test
    fun subscription_required_error_is_recognised() {
        assertEquals(
            WarrenForumLoginOutcome.SubscriptionRequired,
            parseForumLoginOutcome("""{"ok":false,"error":"subscription-required"}"""),
        )
    }

    @Test
    fun clock_skew_error_is_recognised() {
        // The one failure the user can repair themselves: the JNI maps
        // connect's 401 clock_skew token to this envelope error, and the UI
        // must tell the user to fix the device clock rather than "try again",
        // which is what every 2026-08-18 reporter was left with.
        assertEquals(
            WarrenForumLoginOutcome.ClockSkew,
            parseForumLoginOutcome("""{"ok":false,"error":"clock-skew"}"""),
        )
    }

    @Test
    fun any_other_error_is_a_generic_failure() {
        assertTrue(
            parseForumLoginOutcome("""{"ok":false,"error":"error"}""")
                is WarrenForumLoginOutcome.Failure,
        )
    }

    @Test
    fun malformed_json_is_a_failure_not_a_crash() {
        assertTrue(parseForumLoginOutcome("not json") is WarrenForumLoginOutcome.Failure)
    }

    @Test
    fun the_prompt_picks_the_message_matching_the_outcome() {
        assertEquals(
            "fix-clock",
            failureMessageFor(WarrenForumLoginOutcome.ClockSkew, "sub", "wallet", "fix-clock", "expired", "busy", "generic"),
        )
        assertEquals(
            "sub",
            failureMessageFor(WarrenForumLoginOutcome.SubscriptionRequired, "sub", "wallet", "fix-clock", "expired", "busy", "generic"),
        )
        assertEquals(
            "wallet",
            failureMessageFor(WarrenForumLoginOutcome.WalletNotReady, "sub", "wallet", "fix-clock", "expired", "busy", "generic"),
        )
        assertEquals(
            "generic",
            failureMessageFor(WarrenForumLoginOutcome.Failure("x"), "sub", "wallet", "fix-clock", "expired", "busy", "generic"),
        )
        // A deferral is not a failure of the session: its own message, and
        // the prompt stays armed for the retry.
        val deferred = WarrenForumLoginOutcome.Deferred("connecting")
        assertEquals("busy", failureMessageFor(deferred, "sub", "wallet", "fix-clock", "expired", "busy", "generic"))
        assertFalse(isTerminalOutcome(deferred))
        assertEquals("deferred-connecting", outcomeClass(deferred))
    }

    @Test
    fun a_clock_skew_refusal_ends_the_pending_link() {
        // The provider cancels the session on the skewed attempt, so a retry on
        // the same sid can only answer "unknown session". Keeping Approve live
        // would walk the user, who just fixed the clock as told, straight back
        // into the generic dead end.
        assertTrue(isTerminalOutcome(WarrenForumLoginOutcome.ClockSkew))
        assertTrue(isTerminalOutcome(WarrenForumLoginOutcome.SubscriptionRequired))
        assertFalse(isTerminalOutcome(WarrenForumLoginOutcome.WalletNotReady))
        assertFalse(isTerminalOutcome(WarrenForumLoginOutcome.Failure("transient")))
    }

    @Test
    fun an_approved_envelope_carries_the_forum_identity_when_present() {
        // The desktop reads the handle and the digest slot out of the approved
        // body; the JNI envelope now carries both so Android learns its forum
        // name the same way. A slot is optional, a handle absent means unknown.
        assertEquals(
            WarrenForumLoginOutcome.Approved(
                com.warrenbrowse.vpn.lib.model.forum.ForumIdentity("lusab-babad-dovok", 42)
            ),
            parseForumLoginOutcome("""{"ok":true,"handle":"lusab-babad-dovok","notify_slot":42}"""),
        )
        assertEquals(
            WarrenForumLoginOutcome.Approved(
                com.warrenbrowse.vpn.lib.model.forum.ForumIdentity("lusab-babad-dovok", null)
            ),
            parseForumLoginOutcome("""{"ok":true,"handle":"lusab-babad-dovok"}"""),
        )
    }

    @Test
    fun an_expired_session_is_named_and_ends_the_pending_link() {
        assertEquals(
            WarrenForumLoginOutcome.Expired,
            parseForumLoginOutcome("""{"ok":false,"error":"expired"}"""),
        )
        assertTrue(isTerminalOutcome(WarrenForumLoginOutcome.Expired))
        assertEquals(
            "expired",
            failureMessageFor(WarrenForumLoginOutcome.Expired, "sub", "wallet", "fix-clock", "expired", "busy", "generic"),
        )
    }

    @Test
    fun a_failure_keeps_its_reason_class_for_the_log_and_stays_generic_for_the_user() {
        val outcome = parseForumLoginOutcome("""{"ok":false,"error":"error","reason":"transport"}""")
        assertEquals(WarrenForumLoginOutcome.Failure("transport"), outcome)
        assertEquals("transport", outcomeClass(outcome))
        assertEquals(
            "generic",
            failureMessageFor(outcome, "sub", "wallet", "fix-clock", "expired", "busy", "generic"),
        )
        assertEquals(
            WarrenForumLoginOutcome.Failure("unknown"),
            parseForumLoginOutcome("""{"ok":false,"error":"error"}"""),
        )
    }

    // The cross-platform fixture (fixtures/client-rules/README.md): the JNI
    // envelope of every login outcome, decoded here exactly as the Rust crate
    // emits it, so the two sides of the FFI cannot drift.
    @Test
    fun the_shared_outcome_fixture_replays_every_login_envelope() {
        val login = ClientRulesFixtures.load("forum_outcomes.json")["login"]!!.jsonObject
        val terminalKinds = login["terminal_kinds"]!!.jsonArray.map { it.jsonPrimitive.content }
        val cases = login.cases("cases").filterNot(ClientRulesFixtures::skippedOnAndroid)
        assertTrue(cases.size >= 10, "only ${cases.size} login cases reached this reader")
        for (case in cases) {
            val name = case.string("name")
            val expect = case["expect"]!!.jsonObject
            val kind = expect.string("kind")
            val expected: WarrenForumLoginOutcome =
                when (kind) {
                    "approved" ->
                        WarrenForumLoginOutcome.Approved(
                            expect.stringOrNull("handle")?.let {
                                ForumIdentity(it, expect["notify_slot"]?.jsonPrimitive?.intOrNull)
                            }
                        )
                    "subscription-required" -> WarrenForumLoginOutcome.SubscriptionRequired
                    "clock-skew" -> WarrenForumLoginOutcome.ClockSkew
                    "expired" -> WarrenForumLoginOutcome.Expired
                    "failed" -> WarrenForumLoginOutcome.Failure(expect.string("reason"))
                    else -> error("$name: unknown login kind $kind")
                }
            val outcome = parseForumLoginOutcome(case.string("envelope"))
            assertEquals(expected, outcome, name)
            assertEquals(kind in terminalKinds, isTerminalOutcome(outcome), "$name: terminal")
        }
        val clientSide = login["client_side_failures"]!!.jsonObject.cases("cases")
        assertTrue(clientSide.isNotEmpty())
        for (case in clientSide) {
            assertEquals(
                WarrenForumLoginOutcome.Failure(case.string("reason")),
                parseForumLoginOutcome(case.string("envelope")),
                case.string("name"),
            )
        }
    }
}
