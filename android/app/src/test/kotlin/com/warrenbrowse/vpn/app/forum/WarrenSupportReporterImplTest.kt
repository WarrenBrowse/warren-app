package com.warrenbrowse.vpn.app.forum

import android.content.Context
import com.warrenbrowse.vpn.lib.repository.ForumPreflight
import com.warrenbrowse.vpn.lib.repository.WarrenConnectedInfo
import io.mockk.every
import io.mockk.mockk
import java.io.File
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.test.runTest
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.io.TempDir

class WarrenSupportReporterImplTest {

    private fun reporter(
        dir: File,
        jni: FakeJniBridge,
        journal: ForumEventsJournal,
        tunnel: WarrenConnectedInfo = WarrenConnectedInfo.Disconnected,
    ): WarrenSupportReporterImpl {
        val context = mockk<Context>(relaxed = true)
        every { context.cacheDir } returns dir
        return WarrenSupportReporterImpl(
            context = context,
            jni = jni,
            walletRepository = FakeWalletRepository(),
            forumIdentityRepository = FakeForumIdentityRepository(),
            tunnelState = FakeTunnelStateProvider(tunnel),
            journal = journal,
            appLogDir = dir,
            // The one header line under test, read the way ForumDiagnostics
            // renders it; the platform readers need a device.
            facts = ForumFacts { _, _, lastLoginClass -> mapOf("last-forum-login" to (lastLoginClass ?: "none")) },
        )
    }

    @Test
    fun the_report_header_carries_the_class_of_the_last_sign_in_result(@TempDir dir: File) = runTest {
        val journal = ForumEventsJournal(dir, CoroutineScope(SupervisorJob()))
        journal.record("login.result", "class" to "transport")
        val jni = FakeJniBridge()

        reporter(dir, jni, journal).collect().getOrThrow()

        val metadata = jni.collectedMetadata.single()
        assertTrue(metadata.contains("\"last-forum-login\":\"transport\""), metadata)
    }

    @Test
    fun the_report_header_says_none_before_any_sign_in(@TempDir dir: File) = runTest {
        val journal = ForumEventsJournal(dir, CoroutineScope(SupervisorJob()))
        val jni = FakeJniBridge()

        reporter(dir, jni, journal).collect().getOrThrow()

        assertTrue(jni.collectedMetadata.single().contains("\"last-forum-login\":\"none\""))
    }

    @Test
    fun a_report_while_the_kill_switch_holds_is_deferred_and_journaled(@TempDir dir: File) = runTest {
        val journal = ForumEventsJournal(dir, CoroutineScope(SupervisorJob()))
        val reporter = reporter(dir, FakeJniBridge(), journal, WarrenConnectedInfo.Blocking("held"))

        assertEquals(ForumPreflight.Defer("blocking"), reporter.preflight())
        assertEquals("blocking", journal.lastClassOf("report.deferred"))
    }

    @Test
    fun a_settled_tunnel_journals_nothing_at_preflight(@TempDir dir: File) = runTest {
        val journal = ForumEventsJournal(dir, CoroutineScope(SupervisorJob()))
        val reporter = reporter(dir, FakeJniBridge(), journal, WarrenConnectedInfo.Disconnected)

        assertEquals(ForumPreflight.Proceed, reporter.preflight())
        assertEquals(null, journal.lastClassOf("report.deferred"))
    }
}
