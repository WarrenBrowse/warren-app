package com.warrenbrowse.vpn.app.forum

import com.warrenbrowse.vpn.lib.model.forum.ForumIdentity
import com.warrenbrowse.vpn.lib.repository.ReportSubmitOutcome
import java.time.Instant
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

class ReportOutcomeTest {
    @Test
    fun a_created_envelope_carries_the_topic_the_logs_status_and_the_identity() {
        val outcome =
            parseReportOutcome(
                """{"handle":"lusab-babad-dovok","logs":"attached","notify_slot":7,"ok":true,"topic_id":142,"topic_url":"https://forum.warrenbrowse.com/t/142"}"""
            )
        assertEquals(
            ReportSubmitOutcome.Created(
                topicId = 142,
                topicUrl = "https://forum.warrenbrowse.com/t/142",
                identity = ForumIdentity("lusab-babad-dovok", 7),
                logs = "attached",
            ),
            outcome,
        )
        assertEquals("created-attached", reportOutcomeClass(outcome))
    }

    @Test
    fun every_frozen_error_token_maps_to_its_own_outcome() {
        fun of(token: String) = parseReportOutcome("""{"ok":false,"error":"$token"}""")
        assertEquals(ReportSubmitOutcome.SubscriptionRequired, of("subscription-required"))
        assertEquals(ReportSubmitOutcome.ClockSkew, of("clock-skew"))
        assertEquals(ReportSubmitOutcome.RateLimited, of("rate-limited"))
        assertEquals(ReportSubmitOutcome.TooLarge, of("too-large"))
        assertEquals(ReportSubmitOutcome.Invalid, of("invalid"))
        assertEquals(ReportSubmitOutcome.ServerError, of("server-error"))
        assertEquals(
            ReportSubmitOutcome.Failure("http-418"),
            parseReportOutcome("""{"ok":false,"error":"error","reason":"http-418"}"""),
        )
        assertTrue(parseReportOutcome("nope") is ReportSubmitOutcome.Failure)
    }

    @Test
    fun a_journal_line_is_one_json_object_with_the_sequence_time_and_event_first() {
        val line =
            ForumEventsJournal.format(
                Instant.parse("2026-09-02T18:07:05Z"),
                3,
                "login.result",
                listOf("class" to "transport", "elapsed_ms" to "5012"),
            )
        assertEquals(
            """{"seq":3,"at":"2026-09-02T18:07:05Z","event":"login.result","class":"transport","elapsed_ms":"5012"}""",
            line,
        )
    }
}
