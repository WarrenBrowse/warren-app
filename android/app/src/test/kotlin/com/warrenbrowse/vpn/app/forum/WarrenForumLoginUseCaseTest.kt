package com.warrenbrowse.vpn.app.forum

import com.warrenbrowse.vpn.lib.model.forum.ForumIdentity
import com.warrenbrowse.vpn.lib.repository.WarrenConnectedInfo
import java.io.File
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.test.runTest
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.io.TempDir

class WarrenForumLoginUseCaseTest {

    private val link =
        ForumLoginLink(sid = "0123456789abcdef0123456789abcdef", host = "connect.warrenbrowse.com")

    @Test
    fun approving_while_the_tunnel_comes_up_defers_without_reading_the_wallet_or_calling_rust(
        @TempDir dir: File
    ) = runTest {
        val wallet = FakeWalletRepository()
        val jni = FakeJniBridge()
        val journal = ForumEventsJournal(dir, CoroutineScope(SupervisorJob()))
        val useCase =
            WarrenForumLoginUseCase(
                walletRepository = wallet,
                forumIdentityRepository = FakeForumIdentityRepository(),
                journal = journal,
                jni = jni,
                tunnelState = FakeTunnelStateProvider(WarrenConnectedInfo.Connecting()),
            )

        val outcome = useCase.signIn(link)

        assertEquals(WarrenForumLoginOutcome.Deferred("connecting"), outcome)
        assertEquals(0, jni.loginCalls)
        assertEquals(0, wallet.mnemonicReads)
        assertEquals("connecting", journal.lastClassOf("login.deferred"))
        // The prompt must stay armed: the user retries once the tunnel settles.
        assertFalse(isTerminalOutcome(outcome))
    }

    @Test
    fun approving_with_the_tunnel_up_signs_through_rust_and_keeps_the_identity(@TempDir dir: File) =
        runTest {
            val jni =
                FakeJniBridge(loginAnswer = { """{"ok":true,"handle":"lusab-babad-dovok","notify_slot":3}""" })
            val identities = FakeForumIdentityRepository()
            val useCase =
                WarrenForumLoginUseCase(
                    walletRepository = FakeWalletRepository(),
                    forumIdentityRepository = identities,
                    journal = ForumEventsJournal(dir, CoroutineScope(SupervisorJob())),
                    jni = jni,
                    tunnelState =
                        FakeTunnelStateProvider(
                            WarrenConnectedInfo.Connected("203.0.113.7:443", null, false, false, null)
                        ),
                )

            val outcome = useCase.signIn(link)

            assertEquals(
                WarrenForumLoginOutcome.Approved(ForumIdentity("lusab-babad-dovok", 3)),
                outcome,
            )
            assertEquals(1, jni.loginCalls)
            assertEquals(ForumIdentity("lusab-babad-dovok", 3), identities.identity.value)
        }
}
