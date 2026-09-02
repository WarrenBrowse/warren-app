package com.warrenbrowse.vpn.app.forum

import com.warrenbrowse.vpn.lib.model.forum.ForumIdentity
import com.warrenbrowse.vpn.lib.repository.ReportSubmitOutcome
import java.io.File
import java.time.Instant
import java.util.concurrent.CountDownLatch
import kotlin.concurrent.thread
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.test.runTest
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.long
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.io.TempDir

class ForumEventsJournalTest {

    private fun journal(dir: File) = ForumEventsJournal(dir, CoroutineScope(SupervisorJob()))

    private val sid = "a1b2c3d4e5f60718293a4b5c6d7e8f90"

    @Test
    fun the_class_of_the_last_journaled_event_of_a_kind_is_read_back(@TempDir dir: File) = runTest {
        val journal = journal(dir)
        journal.record(
            ForumEvent.LOGIN_RESULT,
            JournalField.Class("transport"),
            JournalField.ElapsedMs(5012),
        )
        journal.record(ForumEvent.LOGIN_DEFERRED, JournalField.Class("connecting"))
        journal.record(ForumEvent.LOGIN_RESULT, JournalField.Class("expired"))

        assertEquals("expired", journal.lastClassOf(ForumEvent.LOGIN_RESULT))
        assertEquals("connecting", journal.lastClassOf(ForumEvent.LOGIN_DEFERRED))
    }

    @Test
    fun lines_recorded_from_many_threads_at_once_carry_distinct_consecutive_sequence_numbers(
        @TempDir dir: File
    ) = runTest {
        // A submit on IO and a deep link on main journal at the same time; the
        // staff order the attempts by `seq`, so two lines must never share one.
        val journal = journal(dir)
        val threads = 8
        val perThread = 200
        val start = CountDownLatch(1)
        val workers =
            List(threads) {
                thread {
                    start.await()
                    repeat(perThread) { journal.record(ForumEvent.LINK_RECEIVED, JournalField.Verdict("ok")) }
                }
            }
        start.countDown()
        workers.forEach { it.join() }
        // Sequenced behind every pending write on the journal's own thread.
        journal.lastClassOf(ForumEvent.LINK_RECEIVED)

        val seqs =
            File(dir, ForumEventsJournal.FILE_NAME).readLines().map {
                Json.parseToJsonElement(it).jsonObject["seq"]!!.jsonPrimitive.long
            }
        assertEquals(threads * perThread, seqs.size)
        assertEquals((0L until (threads * perThread).toLong()).toList(), seqs.sorted())
    }

    @Test
    fun an_event_never_journaled_reads_back_as_null(@TempDir dir: File) = runTest {
        val journal = journal(dir)
        assertNull(journal.lastClassOf(ForumEvent.LOGIN_RESULT))
        journal.record(ForumEvent.REPORT_SUBMIT, JournalField.Class("created-attached"))
        assertNull(journal.lastClassOf(ForumEvent.LOGIN_RESULT))
    }

    @Test
    fun a_corrupt_line_never_stops_the_readback(@TempDir dir: File) = runTest {
        // A truncated tail is what a process killed mid-write leaves behind.
        val good =
            ForumEventsJournal.format(
                Instant.parse("2026-09-02T18:07:05Z"),
                0,
                ForumEvent.LOGIN_RESULT,
                listOf(JournalField.Class("clock-skew")),
            )
        File(dir, ForumEventsJournal.FILE_NAME).writeText(good + "\n{\"seq\":1,\"event\":\"login.res")

        assertEquals("clock-skew", journal(dir).lastClassOf(ForumEvent.LOGIN_RESULT))
    }

    @Test
    fun a_journal_past_its_cap_keeps_its_newest_half_and_the_new_line(@TempDir dir: File) = runTest {
        val file = File(dir, ForumEventsJournal.FILE_NAME)
        val old =
            List(4000) { i ->
                ForumEventsJournal.format(
                    Instant.parse("2026-09-02T18:07:05Z"),
                    i.toLong(),
                    ForumEvent.LOGIN_RESULT,
                    listOf(JournalField.Class("transport"), JournalField.ElapsedMs(5000L + i)),
                )
            }
        file.writeText(old.joinToString("\n", postfix = "\n"))
        assertTrue(file.length() > ForumEventsJournal.MAX_BYTES)
        val journal = journal(dir)

        journal.record(ForumEvent.LOGIN_RESULT, JournalField.Class("expired"))

        // The readback is sequenced behind the write on the journal's thread.
        assertEquals("expired", journal.lastClassOf(ForumEvent.LOGIN_RESULT))
        val lines = file.readLines()
        assertEquals(2001, lines.size)
        assertEquals(old.drop(2000), lines.dropLast(1))
        assertTrue(lines.last().contains("\"class\":\"expired\""))
        assertTrue(file.length() < ForumEventsJournal.MAX_BYTES)
    }

    @Test
    fun a_session_id_handed_to_a_class_field_is_journaled_as_malformed(@TempDir dir: File) = runTest {
        val journal = journal(dir)

        journal.record(ForumEvent.LOGIN_RESULT, JournalField.Class(sid))
        journal.record(ForumEvent.LINK_RECEIVED, JournalField.Verdict(sid), JournalField.Referrer(sid))
        journal.record(ForumEvent.REPORT_COLLECT, JournalField.Reason(TEST_ADDRESS))

        assertEquals("malformed", journal.lastClassOf(ForumEvent.LOGIN_RESULT))
        val text = File(dir, ForumEventsJournal.FILE_NAME).readText()
        assertFalse(text.contains(sid), text)
        assertFalse(text.contains(TEST_ADDRESS), text)
        assertEquals(4, Regex("\"malformed\"").findAll(text).count())
    }

    @Test
    fun a_referrer_is_a_dotted_host_or_none() {
        assertEquals("org.mozilla.firefox", JournalField.Referrer("org.mozilla.firefox").value)
        assertEquals("forum.warrenbrowse.com", JournalField.Referrer("forum.warrenbrowse.com").value)
        assertEquals("none", JournalField.Referrer(null).value)
        assertEquals("malformed", JournalField.Referrer("firefox").value)
    }

    @Test
    fun every_class_the_app_can_journal_fits_the_class_grammar() {
        val tunnelClasses = listOf("connecting", "reconnecting", "disconnecting", "blocking", "failed")
        val loginOutcomes =
            listOf(
                WarrenForumLoginOutcome.Approved(ForumIdentity("lusab-babad-dovok", 3)),
                WarrenForumLoginOutcome.Approved(null),
                WarrenForumLoginOutcome.SubscriptionRequired,
                WarrenForumLoginOutcome.ClockSkew,
                WarrenForumLoginOutcome.Expired,
                WarrenForumLoginOutcome.WalletNotReady,
                WarrenForumLoginOutcome.Failure("transport"),
                WarrenForumLoginOutcome.Failure("http-502"),
                WarrenForumLoginOutcome.Failure("invalid-envelope"),
            ) + tunnelClasses.map { WarrenForumLoginOutcome.Deferred(it) }
        val reportOutcomes =
            listOf(
                ReportSubmitOutcome.Created(1, "", null, "attached"),
                ReportSubmitOutcome.Created(1, "", null, "partial"),
                ReportSubmitOutcome.Created(1, "", null, "none"),
                ReportSubmitOutcome.SubscriptionRequired,
                ReportSubmitOutcome.ClockSkew,
                ReportSubmitOutcome.RateLimited,
                ReportSubmitOutcome.TooLarge,
                ReportSubmitOutcome.Invalid,
                ReportSubmitOutcome.ServerError,
                ReportSubmitOutcome.WalletNotReady,
                ReportSubmitOutcome.Failure("runtime"),
            ) + tunnelClasses.map { ReportSubmitOutcome.Deferred(it) }
        val linkVerdicts =
            listOf(
                "accepted",
                "no-data",
                "not-a-uri",
                "wrong-scheme:https",
                "wrong-scheme:none",
                "wrong-action",
                "missing-sid",
                "missing-host",
                "bad-sid-shape",
                "host-not-allowlisted",
            )
        val stepClasses = listOf("wallet-absent", "wallet-read", "jni", "gzip", "ok", "failed")
        val tokens =
            loginOutcomes.map(::outcomeClass) +
                reportOutcomes.map(::reportOutcomeClass) +
                linkVerdicts +
                stepClasses +
                tunnelClasses

        for (token in tokens) {
            assertEquals(token, JournalField.Class(token).value, "class $token")
        }
        assertEquals("write", JournalField.Reason(collectReasonToken("collect failed: write")).value)
        assertEquals(
            "notfound",
            JournalField.Reason(collectReasonToken("report unreadable: NotFound")).value,
        )
    }
}
