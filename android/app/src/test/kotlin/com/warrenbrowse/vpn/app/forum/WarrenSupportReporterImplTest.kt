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
import java.util.Base64
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

        reporter(dir, jni, journal).collect(forSend = false).getOrThrow()

        val metadata = jni.collectedMetadata.single()
        assertTrue(metadata.contains("\"last-forum-login\":\"transport\""), metadata)
    }

    @Test
    fun the_report_header_says_none_before_any_sign_in(@TempDir dir: File) = runTest {
        val jni = FakeJniBridge()

        reporter(dir, jni).collect(forSend = false).getOrThrow()

        assertTrue(jni.collectedMetadata.single().contains("\"last-forum-login\":\"none\""))
    }

    @Test
    fun whether_the_collection_is_for_a_send_reaches_the_collector(@TempDir dir: File) = runTest {
        // The Rust collector runs the network probes on that flag alone.
        val jni = FakeJniBridge()

        reporter(dir, jni).collect(forSend = false).getOrThrow()
        reporter(dir, jni).collect(forSend = true).getOrThrow()

        assertEquals(listOf(false, true), jni.collectedForSend)
    }

    @Test
    fun a_collection_prunes_the_stale_files_of_the_report_directory_and_keeps_the_fresh_ones(
        @TempDir dir: File
    ) = runTest {
        val reports = File(dir, WarrenSupportReporterImpl.REPORT_DIR).apply { mkdirs() }
        val sharedLongAgo =
            File(reports, "shared-warren-report-old.log").apply {
                writeText("shared")
                setLastModified(System.currentTimeMillis() - 2 * WarrenSupportReporterImpl.STALE_REPORT_MILLIS)
            }
        val sharedJustNow = File(reports, "shared-warren-report-new.log").apply { writeText("shared") }

        reporter(dir).collect(forSend = false).getOrThrow()

        assertFalse(sharedLongAgo.exists(), "an hour-old share copy is pruned")
        assertTrue(sharedJustNow.exists(), "a copy a receiver may still be reading is kept")
    }

    @Test
    fun the_report_header_carries_the_tunnel_state_word_without_its_detail(@TempDir dir: File) =
        runTest {
            val jni = FakeJniBridge()

            reporter(
                    dir,
                    jni,
                    tunnel = WarrenConnectedInfo.Failed("exit 203.0.113.7:443 refused"),
                    stateText = "Failed: exit 203.0.113.7:443 refused",
                )
                .collect(forSend = false)
                .getOrThrow()

            val metadata = jni.collectedMetadata.single()
            assertTrue(metadata.contains("\"tunnel-state\":\"Failed\""), metadata)
            assertFalse(metadata.contains("203.0.113.7"), metadata)
        }

    @Test
    fun the_report_header_never_carries_the_forwarded_port_of_a_connected_tunnel(@TempDir dir: File) =
        runTest {
            // The display string has no colon in its Connected shape, so a cut
            // at the colon kept the whole "Connected (mimicry, port 41231)":
            // the port the exit assigned, in a report the wallet signs.
            val jni = FakeJniBridge()

            reporter(
                    dir,
                    jni,
                    tunnel =
                        WarrenConnectedInfo.Connected(
                            exitEndpointHost = "203.0.113.7:443",
                            entryEndpointHost = null,
                            multiHop = false,
                            daita = false,
                            assignedNatPmpPort = 41231,
                        ),
                    stateText = "Connected (mimicry, port 41231)",
                )
                .collect(forSend = false)
                .getOrThrow()

            val metadata = jni.collectedMetadata.single()
            assertTrue(metadata.contains("\"tunnel-state\":\"Connected\""), metadata)
            assertFalse(metadata.contains("41231"), metadata)
            assertFalse(metadata.contains("mimicry"), metadata)
        }

    @Test
    fun every_tunnel_state_maps_to_one_capitalised_word() {
        val connecting = WarrenConnectedInfo.Connecting(exitEndpointHost = "203.0.113.7:443")
        assertEquals("Disconnected", tunnelStateWord(WarrenConnectedInfo.Disconnected))
        assertEquals("Connecting", tunnelStateWord(connecting))
        assertEquals("Reconnecting", tunnelStateWord(WarrenConnectedInfo.Reconnecting()))
        assertEquals("Disconnecting", tunnelStateWord(WarrenConnectedInfo.Disconnecting(reconnecting = true)))
        assertEquals("Blocking", tunnelStateWord(WarrenConnectedInfo.Blocking("exit 203.0.113.7 refused")))
    }

    @Test
    fun a_collector_failure_journals_the_step_and_the_io_kind_only(@TempDir dir: File) = runTest {
        val journal = RecordingJournal()
        val jni = FakeJniBridge(collectAnswer = { """{"ok":false,"error":"report unreadable: NotFound"}""" })

        val result = reporter(dir, jni, journal).collect(forSend = false)

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
    fun a_gzip_at_the_cap_fills_the_brokers_base64_field_exactly() {
        // warren-connect caps the base64 FIELD at 16,000,000 characters, so
        // the byte cap must encode within it (or an at-cap report is uploaded
        // whole and refused) and the next byte must not (or the cap is low).
        val brokerMaxB64Chars = 16_000_000
        assertTrue(Base64.getEncoder().encodeToString(ByteArray(cap)).length <= brokerMaxB64Chars)
        assertTrue(Base64.getEncoder().encodeToString(ByteArray(cap + 1)).length > brokerMaxB64Chars)
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
