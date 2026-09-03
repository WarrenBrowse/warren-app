package com.warrenbrowse.vpn.app.forum

import com.warrenbrowse.vpn.lib.model.forum.ForumHeaderButton
import com.warrenbrowse.vpn.lib.model.forum.ForumIdentity
import com.warrenbrowse.vpn.lib.model.forum.ForumNotificationKind
import com.warrenbrowse.vpn.lib.model.wallet.WalletState
import com.warrenbrowse.vpn.lib.repository.ForumActivityState
import com.warrenbrowse.vpn.lib.repository.ForumNotificationsResult
import com.warrenbrowse.vpn.lib.repository.WarrenConnectedInfo
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.test.runTest
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

class WarrenForumActivityUseCaseTest {

    private class RecordingActivity : ForumActivityState {
        override val unread: StateFlow<Int> = MutableStateFlow(0)
        override val headerButton: StateFlow<ForumHeaderButton> = MutableStateFlow(ForumHeaderButton.NONE)
        val observed = mutableListOf<Int>()
        val digests = mutableListOf<String?>()

        override fun setDigest(counts: String?) {
            digests += counts
        }

        override fun setObservedUnread(unread: Int) {
            observed += unread
        }
    }

    private val wallet = FakeWalletRepository()
    private val identities = FakeForumIdentityRepository().apply { save(ForumIdentity("lusab-babad-dovok", 2)) }
    private val activity = RecordingActivity()
    private val tunnel = FakeTunnelStateProvider()

    private fun useCase(jni: FakeJniBridge) =
        WarrenForumActivityUseCase(wallet, identities, activity, jni, tunnel)

    @Test
    fun a_panel_read_hands_back_the_rows_and_proves_the_unread_count_at_once() = runTest {
        val jni =
            FakeJniBridge(
                notificationsAnswer = {
                    """{"ok":true,"notifications":[
                        {"id":7,"kind":"replied","unread":true,"created_at":1700000000,"title":"Port forwarding","actor":"rudop-tijub-sozom","path":"/t/86/4"},
                        {"id":8,"kind":"liked","unread":false,"created_at":1700000100}
                    ]}"""
                }
            )

        val result = useCase(jni).list()

        val rows = (result as ForumNotificationsResult.Ok).notifications
        assertEquals(2, rows.size)
        assertEquals(ForumNotificationKind.REPLIED, rows[0].kind)
        assertEquals("/t/86/4", rows[0].path)
        assertEquals(null, rows[1].title)
        assertEquals(1, jni.notificationsCalls)
        assertEquals(1, wallet.mnemonicReads)
        // What the panel just showed is the truth now, not the digest's.
        assertEquals(listOf(1), activity.observed)
    }

    @Test
    fun nothing_leaves_without_a_wallet_or_a_forum_account() = runTest {
        val jni = FakeJniBridge()
        wallet.stateFlow.value = WalletState.Absent

        assertEquals(ForumNotificationsResult.Error("wallet-absent"), useCase(jni).list())
        assertFalse(useCase(jni).markSeen())

        wallet.stateFlow.value = WalletState.Locked(com.warrenbrowse.vpn.lib.model.wallet.WalletAddress(TEST_ADDRESS))
        identities.clear()

        assertEquals(ForumNotificationsResult.Error("no-forum-account"), useCase(jni).list())
        assertEquals(0, jni.notificationsCalls)
        assertEquals(0, wallet.mnemonicReads)
    }

    @Test
    fun a_tunnel_between_states_defers_the_read_before_the_mnemonic_is_touched() = runTest {
        val jni = FakeJniBridge()
        tunnel.info.value = WarrenConnectedInfo.Connecting()

        assertEquals(ForumNotificationsResult.Error("deferred-connecting"), useCase(jni).list())
        assertEquals(0, jni.notificationsCalls)
        assertEquals(0, wallet.mnemonicReads)
    }

    @Test
    fun a_failed_read_is_classed_and_proves_nothing_about_the_count() = runTest {
        val jni = FakeJniBridge(notificationsAnswer = { """{"ok":false,"error":"error","reason":"http-502"}""" })

        assertEquals(ForumNotificationsResult.Error("http-502"), useCase(jni).list())
        assertEquals(emptyList<Int>(), activity.observed)
    }

    @Test
    fun marking_seen_clears_the_badge_first_and_signs_over_its_own_call() = runTest {
        val jni = FakeJniBridge()

        assertTrue(useCase(jni).markSeen())

        assertEquals(listOf(0), activity.observed)
        assertEquals(1, jni.seenCalls)
        assertEquals(0, jni.notificationsCalls)
    }

    @Test
    fun a_refused_mark_seen_reports_false_after_the_optimistic_clear() = runTest {
        // The harmless direction: the list reads as seen here until the next
        // digest puts the count back.
        val jni = FakeJniBridge(seenAnswer = { """{"ok":false,"error":"error","reason":"http-401"}""" })

        assertFalse(useCase(jni).markSeen())
        assertEquals(listOf(0), activity.observed)
    }

    @Test
    fun the_envelope_parser_keeps_unknown_kinds_and_refuses_garbage() {
        val parsed =
            parseForumNotificationsEnvelope(
                """{"ok":true,"notifications":[{"id":1,"kind":"chat_mention","unread":true,"created_at":5},{"id":"x"}]}"""
            )
        val rows = (parsed as ForumNotificationsResult.Ok).notifications
        assertEquals(1, rows.size)
        assertEquals(ForumNotificationKind.OTHER, rows[0].kind)
        assertEquals(ForumNotificationsResult.Error("invalid-envelope"), parseForumNotificationsEnvelope("nope"))
        assertEquals(ForumNotificationsResult.Error("unknown"), parseForumNotificationsEnvelope("""{"ok":false}"""))
        assertTrue(parseSeenEnvelope("""{"ok":true}"""))
        assertFalse(parseSeenEnvelope("""{"ok":false,"error":"error","reason":"transport"}"""))
        assertFalse(parseSeenEnvelope(""))
    }
}
