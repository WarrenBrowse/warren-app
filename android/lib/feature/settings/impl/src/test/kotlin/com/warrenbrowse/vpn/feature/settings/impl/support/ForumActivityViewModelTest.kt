package com.warrenbrowse.vpn.feature.settings.impl.support

import com.warrenbrowse.vpn.lib.common.test.TestCoroutineRule
import com.warrenbrowse.vpn.lib.model.forum.ForumIdentity
import com.warrenbrowse.vpn.lib.model.forum.ForumNotification
import com.warrenbrowse.vpn.lib.model.forum.ForumNotificationKind
import com.warrenbrowse.vpn.lib.repository.ForumIdentityRepository
import com.warrenbrowse.vpn.lib.repository.ForumNotificationsReader
import com.warrenbrowse.vpn.lib.repository.ForumNotificationsResult
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertIs
import kotlin.test.assertTrue
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.test.runTest
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.extension.ExtendWith

@ExtendWith(TestCoroutineRule::class)
class ForumActivityViewModelTest {

    private class FakeReader(private val answer: () -> ForumNotificationsResult) : ForumNotificationsReader {
        var lists = 0
        var seen = 0

        override suspend fun list(): ForumNotificationsResult {
            lists++
            return answer()
        }

        override suspend fun markSeen(): Boolean {
            seen++
            return true
        }
    }

    private class FakeIdentities : ForumIdentityRepository {
        private val flow = MutableStateFlow<ForumIdentity?>(ForumIdentity("lusab-babad-dovok", 2))
        override val identity: StateFlow<ForumIdentity?> = flow.asStateFlow()

        override fun save(identity: ForumIdentity) {
            flow.value = identity
        }

        override fun clear() {
            flow.value = null
        }
    }

    private fun row(id: Long, unread: Boolean) =
        ForumNotification(id, ForumNotificationKind.REPLIED, unread, 1_700_000_000, "Topic", "actor", null, "/t/1/$id")

    @Test
    fun the_panel_reads_once_when_it_opens_and_shows_the_rows() = runTest {
        val reader = FakeReader { ForumNotificationsResult.Ok(listOf(row(1, true), row(2, false))) }

        val viewModel = ForumActivityViewModel(reader, FakeIdentities())

        val ready = assertIs<ForumActivityUiState.Ready>(viewModel.state.value)
        assertEquals(2, ready.notifications.size)
        assertTrue(ready.hasUnread)
        assertEquals(1, reader.lists)
        assertEquals("lusab-babad-dovok", viewModel.handle.value)
    }

    @Test
    fun a_failed_read_is_an_error_the_user_can_retry() = runTest {
        var fail = true
        val reader =
            FakeReader {
                if (fail) ForumNotificationsResult.Error("transport") else ForumNotificationsResult.Ok(emptyList())
            }
        val viewModel = ForumActivityViewModel(reader, FakeIdentities())
        assertIs<ForumActivityUiState.Error>(viewModel.state.value)

        fail = false
        viewModel.reload()

        assertIs<ForumActivityUiState.Ready>(viewModel.state.value)
        assertEquals(2, reader.lists)
    }

    @Test
    fun mark_all_read_repaints_before_the_round_trip_and_marks_the_list_seen() = runTest {
        val reader = FakeReader { ForumNotificationsResult.Ok(listOf(row(1, true), row(2, true))) }
        val viewModel = ForumActivityViewModel(reader, FakeIdentities())

        viewModel.markAllRead()

        val ready = assertIs<ForumActivityUiState.Ready>(viewModel.state.value)
        assertFalse(ready.hasUnread)
        assertEquals(1, reader.seen)
    }

    @Test
    fun opening_one_card_marks_only_that_card_read_locally() = runTest {
        val reader = FakeReader { ForumNotificationsResult.Ok(listOf(row(1, true), row(2, true))) }
        val viewModel = ForumActivityViewModel(reader, FakeIdentities())

        viewModel.markOneRead(1)

        val ready = assertIs<ForumActivityUiState.Ready>(viewModel.state.value)
        assertEquals(listOf(false, true), ready.notifications.map { it.unread })
        assertEquals(0, reader.seen)
    }
}
