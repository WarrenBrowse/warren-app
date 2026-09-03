package com.warrenbrowse.vpn.lib.repository

import com.warrenbrowse.vpn.lib.model.forum.ForumHeaderButton
import com.warrenbrowse.vpn.lib.model.forum.ForumIdentity
import com.warrenbrowse.vpn.lib.model.forum.ForumNotification
import com.warrenbrowse.vpn.lib.model.forum.forumHeaderButton
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.launch

/**
 * The forum activity badge as every surface reads it: the unread count and
 * the header slot's verdict, computed once so the bell, the notification and
 * the panel cannot drift apart, and corrected the instant the user acts.
 */
interface ForumActivityState {
    /** Unread forum notifications for this installation; zero whenever there is nothing to show. */
    val unread: StateFlow<Int>

    /** Which button the header's forum slot carries. */
    val headerButton: StateFlow<ForumHeaderButton>

    /** A verified digest as Rust handed it over, or null while no fresh one is held. */
    fun setDigest(counts: String?)

    /** What a panel read or a mark-seen just proved, effective immediately. */
    fun setObservedUnread(unread: Int)
}

/** The platform's notification surface for forum activity. */
interface ForumActivityAlerts {
    /** A rise above what this run had accounted for. */
    fun announce(unread: Int)

    /** Nothing is waiting any more: whatever was posted comes down. */
    fun clear()
}

/** Outcome of one panel read. */
sealed interface ForumNotificationsResult {
    data class Ok(val notifications: List<ForumNotification>) : ForumNotificationsResult

    /** Not attempted, or failed; [reason] is a fixed class for the log, never a value. */
    data class Error(val reason: String) : ForumNotificationsResult
}

/**
 * The account-bound forum calls, made only when the user asks for the
 * content: the panel read, and marking the list seen. The production impl
 * lives in the app module (it needs the JNI bridge and the wallet); the
 * feature screens see only this seam.
 */
interface ForumNotificationsReader {
    suspend fun list(): ForumNotificationsResult

    suspend fun markSeen(): Boolean
}

/**
 * Wires the monitor to what the rest of the app already holds: the slot and
 * the account from the forum identity (cleared with the wallet, so erasing it
 * drops the badge), the setting, and the digest the poller fetches.
 */
class ForumActivityRepository(
    private val identity: StateFlow<ForumIdentity?>,
    private val enabled: StateFlow<Boolean>,
    private val alerts: ForumActivityAlerts,
    private val scope: CoroutineScope,
) : ForumActivityState {
    private val _unread = MutableStateFlow(0)
    override val unread: StateFlow<Int> = _unread.asStateFlow()

    private val _headerButton = MutableStateFlow(ForumHeaderButton.NONE)
    override val headerButton: StateFlow<ForumHeaderButton> = _headerButton.asStateFlow()

    private val monitor =
        ForumActivityMonitor(
            object : ForumActivityMonitor.Delegate {
                override fun notify(unread: Int) = alerts.announce(unread)

                override fun showIndicator(unread: Boolean) {
                    if (!unread) alerts.clear()
                }

                override fun publishUnread(count: Int) {
                    _unread.value = count
                }
            }
        )

    fun start() {
        scope.launch {
            combine(identity, enabled) { identity, enabled -> identity to enabled }
                .collect { (identity, enabled) ->
                    _headerButton.value = forumHeaderButton(identity != null, enabled)
                    monitor.setEnabled(enabled)
                    monitor.setSlot(identity?.notifySlot)
                }
        }
    }

    override fun setDigest(counts: String?) = monitor.setDigest(counts)

    override fun setObservedUnread(unread: Int) = monitor.setObservedUnread(unread)
}
