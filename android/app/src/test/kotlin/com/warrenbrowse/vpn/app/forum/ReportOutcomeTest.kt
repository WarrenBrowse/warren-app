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
        // The one `reason` with its own screen: the resend-without-logs offer.
        val timedOut = parseReportOutcome("""{"ok":false,"error":"error","reason":"upload-timeout"}""")
        assertEquals(ReportSubmitOutcome.UploadTimedOut, timedOut)
        assertEquals("upload-timeout", reportOutcomeClass(timedOut))
        assertTrue(parseReportOutcome("nope") is ReportSubmitOutcome.Failure)
    }

    @Test
    fun a_topic_url_off_the_forum_host_is_dropped_and_the_topic_shown_without_a_link() {
        fun urlOf(topicUrl: String) =
            (parseReportOutcome("""{"ok":true,"topic_id":1,"topic_url":"$topicUrl","logs":"none"}""")
                    as ReportSubmitOutcome.Created)
                .topicUrl
        assertEquals("https://forum.warrenbrowse.com/t/1", urlOf("https://forum.warrenbrowse.com/t/1"))
        // The screen opens the value in the browser on one tap, as a link the
        // app vouched for: a broker answer must not be able to steer it.
        assertEquals("", urlOf("https://evil.example/t/1"))
        assertEquals("", urlOf("https://forum.warrenbrowse.com.evil.example/t/1"))
        assertEquals("", urlOf("http://forum.warrenbrowse.com/t/1"))
        assertEquals("", urlOf("https://forum.warrenbrowse.com@evil.example/t/1"))
        assertEquals("", urlOf("intent://forum.warrenbrowse.com/t/1"))
        assertEquals("", forumTopicUrlOrEmpty(null))
        assertEquals("", forumTopicUrlOrEmpty("not a url at all ://"))
    }

    @Test
    fun a_journal_line_is_one_json_object_with_the_sequence_time_and_event_first() {
        val line =
            ForumEventsJournal.format(
                Instant.parse("2026-09-02T18:07:05Z"),
                3,
                ForumEvent.LOGIN_RESULT,
                listOf(JournalField.Class("transport"), JournalField.ElapsedMs(5012)),
            )
        assertEquals(
            """{"seq":3,"at":"2026-09-02T18:07:05Z","event":"login.result","class":"transport","elapsed_ms":"5012"}""",
            line,
        )
    }
}
