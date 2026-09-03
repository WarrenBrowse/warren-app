package com.warrenbrowse.vpn.lib.repository

import com.warrenbrowse.vpn.lib.model.forum.ForumHeaderButton
import com.warrenbrowse.vpn.lib.model.forum.ForumIdentity
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Test

@OptIn(ExperimentalCoroutinesApi::class)
class ForumActivityRepositoryTest {

    private val identity = MutableStateFlow<ForumIdentity?>(ForumIdentity("lusab-babad-dovok", 2))
    private val enabled = MutableStateFlow(true)
    private val announced = mutableListOf<Int>()
    private var cleared = 0
    private val alerts =
        object : ForumActivityAlerts {
            override fun announce(unread: Int) {
                announced += unread
            }

            override fun clear() {
                cleared++
            }
        }

    @Test
    fun the_wallet_slot_indexes_the_digest_and_erasing_the_identity_drops_the_badge() = runTest {
        val repository = ForumActivityRepository(identity, enabled, alerts, backgroundScope).also { it.start() }
        runCurrent()

        repository.setDigest("002000")
        assertEquals(2, repository.unread.value)
        assertEquals(ForumHeaderButton.ACTIVITY, repository.headerButton.value)

        // The wallet erase clears the identity; the badge and the bell go with
        // it, and the header offers the way into the forum instead.
        identity.value = null
        runCurrent()

        assertEquals(0, repository.unread.value)
        assertEquals(ForumHeaderButton.COMMUNITY, repository.headerButton.value)
    }

    @Test
    fun the_setting_off_removes_the_whole_header_slot() = runTest {
        val repository = ForumActivityRepository(identity, enabled, alerts, backgroundScope).also { it.start() }
        runCurrent()

        enabled.value = false
        runCurrent()

        assertEquals(ForumHeaderButton.NONE, repository.headerButton.value)
        identity.value = null
        runCurrent()
        assertEquals(ForumHeaderButton.NONE, repository.headerButton.value, "lifebuoy included")
    }

    @Test
    fun a_rise_while_watching_is_announced_and_a_read_in_the_panel_clears_it() = runTest {
        val repository = ForumActivityRepository(identity, enabled, alerts, backgroundScope).also { it.start() }
        runCurrent()
        repository.setDigest("000000")

        repository.setDigest("001000")
        assertEquals(listOf(1), announced)

        repository.setObservedUnread(0)
        assertEquals(0, repository.unread.value)
        assertEquals(1, cleared)
    }

    @Test
    fun an_account_without_a_slot_keeps_the_bell_with_nothing_to_count() = runTest {
        // A login before slots existed, or one the allocator had no room for:
        // the forum name is known, the badge simply cannot be computed.
        identity.value = ForumIdentity("lusab-babad-dovok", null)
        val repository = ForumActivityRepository(identity, enabled, alerts, backgroundScope).also { it.start() }
        runCurrent()

        repository.setDigest("fff")

        assertEquals(0, repository.unread.value)
        assertEquals(ForumHeaderButton.ACTIVITY, repository.headerButton.value)
    }
}
