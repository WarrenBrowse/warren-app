package com.warrenbrowse.vpn.app.connect

import com.warrenbrowse.vpn.app.forum.FakeJniBridge
import com.warrenbrowse.vpn.app.forum.FakeWalletRepository
import com.warrenbrowse.vpn.lib.model.wallet.WalletState
import kotlinx.coroutines.test.runTest
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

/**
 * The user-driven incident report behind the key-mismatch dialog's "Report to Warren". The signing
 * and the POST happen in Rust; what this side owns is which facts are handed over, that the
 * recovery phrase is read once and wiped, and that no failure ever reaches the dialog as an
 * exception, since the user has already been told to stay disconnected.
 */
class WarrenIncidentReportUseCaseTest {

    private val exitId = "1a".repeat(16)
    private val pinned = "ab".repeat(32)
    private val observed = "cd".repeat(32)

    private fun report(wallet: FakeWalletRepository, jni: FakeJniBridge) =
        WarrenIncidentReportUseCase(wallet, jni)

    @Test
    fun the_report_carries_both_keys_under_the_exit_id_and_the_published_location() = runTest {
        val wallet = FakeWalletRepository()
        val jni = FakeJniBridge()

        val sent = report(wallet, jni).reportPubkeyMismatch(exitId, pinned, observed, "nl", "Amsterdam")

        assertTrue(sent, "a server that accepted the report is a report that left")
        assertEquals(
            listOf(listOf(exitId, pinned, observed, "nl", "Amsterdam")),
            jni.pubkeyMismatchReports.toList(),
        )
        assertEquals(1, wallet.mnemonicReads, "the phrase is read once, for this one call")
    }

    /** A refused or unreachable report is a lost forensic point, never a thrown error. */
    @Test
    fun a_refused_report_is_answered_and_not_thrown() = runTest {
        val wallet = FakeWalletRepository()
        val jni = FakeJniBridge().apply { pubkeyMismatchAnswer = { """{"ok":false,"reason":"transport"}""" } }

        assertFalse(report(wallet, jni).reportPubkeyMismatch(exitId, pinned, observed, "nl", ""))
    }

    /** Same for a native call that throws: the dialog must still close cleanly. */
    @Test
    fun a_native_failure_is_answered_and_not_thrown() = runTest {
        val wallet = FakeWalletRepository()
        val jni = FakeJniBridge().apply { pubkeyMismatchAnswer = { error("native boom") } }

        assertFalse(report(wallet, jni).reportPubkeyMismatch(exitId, pinned, observed, "", ""))
    }

    /** Without a wallet there is nothing to sign with, so nothing is attempted. */
    @Test
    fun no_wallet_sends_no_report() = runTest {
        val wallet = FakeWalletRepository().apply { stateFlow.value = WalletState.Absent }
        val jni = FakeJniBridge()

        assertFalse(report(wallet, jni).reportPubkeyMismatch(exitId, pinned, observed, "nl", ""))
        assertEquals(emptyList<List<String>>(), jni.pubkeyMismatchReports.toList())
        assertEquals(0, wallet.mnemonicReads)
    }
}
