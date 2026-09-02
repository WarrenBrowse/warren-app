package com.warrenbrowse.vpn.app.forum

import android.content.Context
import com.warrenbrowse.vpn.lib.repository.CollectedReport
import com.warrenbrowse.vpn.lib.repository.ForumPreflight
import com.warrenbrowse.vpn.lib.repository.ReportArea
import com.warrenbrowse.vpn.lib.repository.ReportForm
import com.warrenbrowse.vpn.lib.repository.ReportFrequency
import com.warrenbrowse.vpn.lib.repository.ReportSubmitOutcome
import com.warrenbrowse.vpn.lib.repository.WarrenConnectedInfo
import io.mockk.every
import io.mockk.mockk
import java.io.File
import java.util.zip.GZIPInputStream
import kotlinx.coroutines.test.runTest
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertInstanceOf
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.io.TempDir

class WarrenSupportReporterImplTest {

    private val cap = WarrenSupportReporterImpl.MAX_LOG_GZ_BYTES

    private val form =
        ReportForm(
            area = ReportArea.CONNECTION,
            frequency = ReportFrequency.ALWAYS,
            whatHappened = "The tunnel drops every few minutes on wifi.",
            steps = null,
        )

    private fun reporter(
        dir: File,
        jni: FakeJniBridge = FakeJniBridge(),
        journal: ForumJournal = RecordingJournal(),
        tunnel: WarrenConnectedInfo = WarrenConnectedInfo.Disconnected,
        stateText: String = "Disconnected",
        wallet: FakeWalletRepository = FakeWalletRepository(),
        compressor: LogCompressor = GzipLogCompressor,
    ): WarrenSupportReporterImpl {
        val context = mockk<Context>(relaxed = true)
        every { context.cacheDir } returns dir
        return WarrenSupportReporterImpl(
            context = context,
            jni = jni,
            walletRepository = wallet,
            forumIdentityRepository = FakeForumIdentityRepository(),
            tunnelState = FakeTunnelStateProvider(tunnel, stateText),
            journal = journal,
            appLogDir = dir,
            // The header lines under test, read the way ForumDiagnostics renders
            // them; the platform readers need a device.
            facts =
                ForumFacts { tunnelState, _, lastLoginClass ->
                    mapOf(
                        "tunnel-state" to tunnelState,
                        "last-forum-login" to (lastLoginClass ?: "none"),
                    )
                },
            compressor = compressor,
        )
    }

    private fun collected(dir: File): CollectedReport {
        val file = File(dir, "report.log").apply { writeText("collected") }
        return CollectedReport(file, file.length())
    }

    @Test
    fun the_report_header_carries_the_class_of_the_last_sign_in_result(@TempDir dir: File) = runTest {
        val journal = RecordingJournal()
        journal.record(ForumEvent.LOGIN_RESULT, JournalField.Class("transport"))
        journal.record(ForumEvent.LOGIN_DEFERRED, JournalField.Class("connecting"))
        val jni = FakeJniBridge()

        reporter(dir, jni, journal).collect().getOrThrow()

        val metadata = jni.collectedMetadata.single()
        assertTrue(metadata.contains("\"last-forum-login\":\"transport\""), metadata)
    }

    @Test
    fun the_report_header_says_none_before_any_sign_in(@TempDir dir: File) = runTest {
        val jni = FakeJniBridge()

        reporter(dir, jni).collect().getOrThrow()

        assertTrue(jni.collectedMetadata.single().contains("\"last-forum-login\":\"none\""))
    }

    @Test
    fun the_report_header_carries_the_tunnel_state_word_without_its_detail(@TempDir dir: File) =
        runTest {
            val jni = FakeJniBridge()

            reporter(dir, jni, stateText = "Failed: exit 203.0.113.7:443 refused").collect().getOrThrow()

            val metadata = jni.collectedMetadata.single()
            assertTrue(metadata.contains("\"tunnel-state\":\"Failed\""), metadata)
            assertFalse(metadata.contains("203.0.113.7"), metadata)
        }

    @Test
    fun a_collector_failure_journals_the_step_and_the_io_kind_only(@TempDir dir: File) = runTest {
        val journal = RecordingJournal()
        val jni = FakeJniBridge(collectAnswer = { """{"ok":false,"error":"report unreadable: NotFound"}""" })

        val result = reporter(dir, jni, journal).collect()

        assertTrue(result.isFailure)
        assertEquals(
            listOf(listOf(JournalField.Class("failed"), JournalField.Reason("notfound"))),
            journal.fieldsOf(ForumEvent.REPORT_COLLECT),
        )
    }

    @Test
    fun a_report_while_the_kill_switch_holds_is_deferred_and_journaled(@TempDir dir: File) = runTest {
        val journal = RecordingJournal()
        val reporter = reporter(dir, journal = journal, tunnel = WarrenConnectedInfo.Blocking("held"))

        assertEquals(ForumPreflight.Defer("blocking"), reporter.preflight())
        assertEquals("blocking", journal.lastClassOf(ForumEvent.REPORT_DEFERRED))
    }

    @Test
    fun a_settled_tunnel_journals_nothing_at_preflight(@TempDir dir: File) = runTest {
        val journal = RecordingJournal()
        val reporter = reporter(dir, journal = journal, tunnel = WarrenConnectedInfo.Disconnected)

        assertEquals(ForumPreflight.Proceed, reporter.preflight())
        assertTrue(journal.entries.isEmpty())
    }

    @Test
    fun a_gzip_one_byte_over_the_cap_is_refused_before_the_wallet_or_the_bridge_is_touched(
        @TempDir dir: File
    ) = runTest {
        val jni = FakeJniBridge()
        val wallet = FakeWalletRepository()
        val journal = RecordingJournal()
        val reporter =
            reporter(dir, jni, journal, wallet = wallet, compressor = { ByteArray(cap + 1) })

        val outcome = reporter.submit(form, collected(dir))

        assertEquals(ReportSubmitOutcome.TooLarge, outcome)
        assertEquals(0, jni.reportCalls)
        assertEquals(0, wallet.mnemonicReads)
        assertEquals(
            listOf(listOf(JournalField.Class("too-large"), JournalField.GzBytes(cap + 1L))),
            journal.fieldsOf(ForumEvent.REPORT_SUBMIT),
        )
    }

    @Test
    fun a_gzip_exactly_at_the_cap_is_signed_and_sent(@TempDir dir: File) = runTest {
        val jni = FakeJniBridge()
        val journal = RecordingJournal()
        val reporter = reporter(dir, jni, journal, compressor = { ByteArray(cap) })

        val outcome = reporter.submit(form, collected(dir))

        assertInstanceOf(ReportSubmitOutcome.Created::class.java, outcome)
        assertEquals(1, jni.reportCalls)
        val fields = journal.fieldsOf(ForumEvent.REPORT_SUBMIT).single()
        assertTrue(JournalField.Class("created-none") in fields, fields.toString())
        assertTrue(JournalField.WithLogs(true) in fields, fields.toString())
    }

    @Test
    fun the_default_compressor_gzips_the_report_file(@TempDir dir: File) {
        val content = "header line\n".repeat(2000)
        val file = File(dir, "report.log").apply { writeText(content) }

        val gz = GzipLogCompressor.compress(file)

        assertEquals(0x1f, gz[0].toInt() and 0xff)
        assertEquals(0x8b, gz[1].toInt() and 0xff)
        assertTrue(gz.size < content.length / 10, "gzip did not compress: ${gz.size}")
        assertEquals(content, GZIPInputStream(gz.inputStream()).readBytes().decodeToString())
    }
}
