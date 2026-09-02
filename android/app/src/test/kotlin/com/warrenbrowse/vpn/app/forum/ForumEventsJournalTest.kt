package com.warrenbrowse.vpn.app.forum

import java.io.File
import java.time.Instant
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.test.runTest
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.io.TempDir

class ForumEventsJournalTest {

    private fun journal(dir: File) = ForumEventsJournal(dir, CoroutineScope(SupervisorJob()))

    @Test
    fun the_class_of_the_last_journaled_event_of_a_kind_is_read_back(@TempDir dir: File) = runTest {
        val journal = journal(dir)
        journal.record("login.result", "class" to "transport", "elapsed_ms" to "5012")
        journal.record("login.deferred", "class" to "connecting")
        journal.record("login.result", "class" to "expired")

        assertEquals("expired", journal.lastClassOf("login.result"))
        assertEquals("connecting", journal.lastClassOf("login.deferred"))
    }

    @Test
    fun an_event_never_journaled_reads_back_as_null(@TempDir dir: File) = runTest {
        val journal = journal(dir)
        assertNull(journal.lastClassOf("login.result"))
        journal.record("report.submit", "class" to "created-attached")
        assertNull(journal.lastClassOf("login.result"))
    }

    @Test
    fun a_corrupt_line_never_stops_the_readback(@TempDir dir: File) = runTest {
        // A truncated tail is what a process killed mid-write leaves behind.
        val good = ForumEventsJournal.format(Instant.parse("2026-09-02T18:07:05Z"), 0, "login.result", listOf("class" to "clock-skew"))
        File(dir, ForumEventsJournal.FILE_NAME).writeText(good + "\n{\"seq\":1,\"event\":\"login.res")

        assertEquals("clock-skew", journal(dir).lastClassOf("login.result"))
    }
}
