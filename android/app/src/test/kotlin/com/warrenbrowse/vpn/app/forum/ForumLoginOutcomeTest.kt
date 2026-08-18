package com.warrenbrowse.vpn.app.forum

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

class ForumLoginOutcomeTest {

    @Test
    fun ok_maps_to_approved() {
        assertEquals(WarrenForumLoginOutcome.Approved, parseForumLoginOutcome("""{"ok":true}"""))
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
            failureMessageFor(WarrenForumLoginOutcome.ClockSkew, "sub", "wallet", "fix-clock", "generic"),
        )
        assertEquals(
            "generic",
            failureMessageFor(WarrenForumLoginOutcome.Failure("x"), "sub", "wallet", "fix-clock", "generic"),
        )
    }
}
