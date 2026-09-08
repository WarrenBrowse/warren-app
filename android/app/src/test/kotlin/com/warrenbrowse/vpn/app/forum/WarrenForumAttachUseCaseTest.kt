package com.warrenbrowse.vpn.app.forum

import com.warrenbrowse.vpn.lib.model.wallet.WalletState
import com.warrenbrowse.vpn.lib.repository.WarrenConnectedInfo
import java.io.File
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.test.runTest
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.io.TempDir

class WarrenForumAttachUseCaseTest {

    private val sid = "0123456789abcdef0123456789abcdef"
    private val link = ForumAttachLink(sid = sid, host = "connect.warrenbrowse.com", topicId = 42L)
    private val connected = WarrenConnectedInfo.Connected("203.0.113.7:443", null, false, false, null)

    /** The fakes one attach runs against, each readable after the fact. */
    private class Harness(
        dir: File,
        tunnel: WarrenConnectedInfo,
        val jni: FakeJniBridge = FakeJniBridge(),
        val wallet: FakeWalletRepository = FakeWalletRepository(),
        compressor: LogCompressor = GzipLogCompressor,
    ) {
        val reporter = FakeSupportReporter(dir)
        val journal = RecordingJournal()
        val useCase =
            WarrenForumAttachUseCase(
                walletRepository = wallet,
                reporter = reporter,
                journal = journal,
                jni = jni,
                tunnelState = FakeTunnelStateProvider(tunnel),
                compressor = compressor,
                // Unconfined: the cancel notify runs to completion inside the test.
                cancelScope = CoroutineScope(Dispatchers.Unconfined),
            )
    }

    @Test
    fun approving_with_the_tunnel_up_collects_for_the_send_gzips_and_signs_through_rust(@TempDir dir: File) =
        runTest {
            val h = Harness(dir, connected)

            val outcome = h.useCase.attach(link, topicId = 42L)

            assertEquals(WarrenForumAttachOutcome.Attached, outcome)
            assertEquals(1, h.jni.attachCalls)
            assertEquals(listOf(42L), h.jni.attachedTopics)
            assertTrue(h.jni.attachedGzBytes.single() > 0, "the gzip crossed into Rust")
            // The report describes the moment of the send, probes included.
            assertEquals(listOf(true), h.reporter.collectedForSend)
            assertEquals(1, h.reporter.discarded.size, "the collected file is deleted after the send")
            assertEquals("attached", h.journal.lastClassOf(ForumEvent.ATTACH_RESULT))
            assertTrue(h.journal.fieldsOf(ForumEvent.ATTACH_SIGNING).single().contains(JournalField.PreTopic(false)))
        }

    @Test
    fun a_pre_topic_approval_crosses_with_topic_zero_and_says_so_in_the_journal(@TempDir dir: File) =
        runTest {
            val h = Harness(dir, connected)

            h.useCase.attach(link.copy(topicId = 0L), topicId = 0L)

            assertEquals(listOf(0L), h.jni.attachedTopics)
            assertTrue(h.journal.fieldsOf(ForumEvent.ATTACH_SIGNING).single().contains(JournalField.PreTopic(true)))
        }

    @Test
    fun approving_while_the_tunnel_comes_up_defers_without_collecting_or_reading_the_wallet(
        @TempDir dir: File
    ) = runTest {
        val h = Harness(dir, WarrenConnectedInfo.Connecting())

        val outcome = h.useCase.attach(link, topicId = 42L)

        assertEquals(WarrenForumAttachOutcome.Deferred("connecting"), outcome)
        assertEquals(0, h.reporter.collectCalls)
        assertEquals(0, h.wallet.mnemonicReads)
        assertEquals(0, h.jni.attachCalls)
        assertEquals("connecting", h.journal.lastClassOf(ForumEvent.ATTACH_DEFERRED))
        assertFalse(isTerminalAttachOutcome(outcome), "the prompt stays armed for a retry")
    }

    @Test
    fun no_wallet_refuses_before_anything_is_collected(@TempDir dir: File) = runTest {
        val h = Harness(dir, connected, wallet = FakeWalletRepository(WalletState.Absent))

        val outcome = h.useCase.attach(link, topicId = 42L)

        assertEquals(WarrenForumAttachOutcome.WalletNotReady, outcome)
        assertEquals(0, h.reporter.collectCalls)
        assertEquals(0, h.jni.attachCalls)
        assertEquals("wallet-absent", h.journal.lastClassOf(ForumEvent.ATTACH_RESULT))
    }

    @Test
    fun a_report_over_the_cap_is_refused_before_the_wallet_is_read(@TempDir dir: File) = runTest {
        // The first leg of the size chain: a gzip the broker would refuse
        // never reaches the wallet, Rust, or the network.
        val h =
            Harness(
                dir,
                connected,
                compressor = LogCompressor { ByteArray(WarrenSupportReporterImpl.MAX_LOG_GZ_BYTES + 1) },
            )

        val outcome = h.useCase.attach(link, topicId = 42L)

        assertEquals(WarrenForumAttachOutcome.TooLarge, outcome)
        assertEquals(0, h.wallet.mnemonicReads)
        assertEquals(0, h.jni.attachCalls)
        assertEquals(1, h.reporter.discarded.size)
        val fields = h.journal.fieldsOf(ForumEvent.ATTACH_RESULT).single()
        assertTrue(fields.contains(JournalField.Class("too-large")))
        assertTrue(fields.any { it is JournalField.GzBytes })
    }

    @Test
    fun a_failed_collection_is_its_own_class_and_reaches_no_host(@TempDir dir: File) = runTest {
        val h = Harness(dir, connected)
        h.reporter.collectAnswer = { Result.failure(IllegalStateException("collect failed: write")) }

        val outcome = h.useCase.attach(link, topicId = 42L)

        assertEquals(WarrenForumAttachOutcome.Failure("collect-failed"), outcome)
        assertEquals(0, h.wallet.mnemonicReads)
        assertEquals(0, h.jni.attachCalls)
        assertEquals("collect-failed", h.journal.lastClassOf(ForumEvent.ATTACH_RESULT))
    }

    @Test
    fun declining_tells_the_provider_and_journals_it(@TempDir dir: File) = runTest {
        val h = Harness(dir, connected)

        h.useCase.cancel(link)

        assertEquals(1, h.jni.attachCancelCalls)
        assertEquals(0, h.jni.attachCalls)
        assertTrue(h.journal.entries.any { it.first == ForumEvent.ATTACH_DECLINED })
    }

    @Test
    fun the_envelope_maps_to_the_desktop_result_classes() {
        assertEquals(WarrenForumAttachOutcome.Attached, parseForumAttachOutcome("""{"ok":true}"""))
        assertEquals(
            WarrenForumAttachOutcome.NotAuthor,
            parseForumAttachOutcome("""{"ok":false,"error":"not-author"}"""),
        )
        assertEquals(WarrenForumAttachOutcome.Expired, parseForumAttachOutcome("""{"ok":false,"error":"expired"}"""))
        assertEquals(WarrenForumAttachOutcome.TooLarge, parseForumAttachOutcome("""{"ok":false,"error":"too-large"}"""))
        assertEquals(WarrenForumAttachOutcome.ClockSkew, parseForumAttachOutcome("""{"ok":false,"error":"clock-skew"}"""))
        assertEquals(
            WarrenForumAttachOutcome.ServerError,
            parseForumAttachOutcome("""{"ok":false,"error":"server-error"}"""),
        )
        assertEquals(
            WarrenForumAttachOutcome.Failure("transport"),
            parseForumAttachOutcome("""{"ok":false,"error":"error","reason":"transport"}"""),
        )
        assertEquals(
            WarrenForumAttachOutcome.Failure("http-418"),
            parseForumAttachOutcome("""{"ok":false,"error":"error","reason":"http-418"}"""),
        )
        assertEquals(WarrenForumAttachOutcome.Failure("invalid-envelope"), parseForumAttachOutcome("not json"))
        assertEquals(WarrenForumAttachOutcome.Failure("unknown"), parseForumAttachOutcome("""{"ok":false}"""))
    }

    @Test
    fun every_attach_class_fits_the_journal_grammar() {
        val outcomes =
            listOf(
                WarrenForumAttachOutcome.Attached,
                WarrenForumAttachOutcome.NotAuthor,
                WarrenForumAttachOutcome.Expired,
                WarrenForumAttachOutcome.TooLarge,
                WarrenForumAttachOutcome.ClockSkew,
                WarrenForumAttachOutcome.ServerError,
                WarrenForumAttachOutcome.WalletNotReady,
                WarrenForumAttachOutcome.Deferred("reconnecting"),
                WarrenForumAttachOutcome.Failure("collect-failed"),
                WarrenForumAttachOutcome.Failure("upload-timeout"),
                WarrenForumAttachOutcome.Failure("http-502"),
            )
        for (outcome in outcomes) {
            val token = attachOutcomeClass(outcome)
            assertEquals(token, JournalField.Class(token).value, "class $token")
        }
    }
}
