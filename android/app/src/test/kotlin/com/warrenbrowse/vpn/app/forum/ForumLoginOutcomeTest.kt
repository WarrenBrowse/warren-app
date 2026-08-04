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
}
