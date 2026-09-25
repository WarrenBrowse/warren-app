package com.warrenbrowse.vpn.lib.pushnotification.standing

import com.warrenbrowse.vpn.lib.model.Notification
import com.warrenbrowse.vpn.lib.model.NotificationChannelId
import com.warrenbrowse.vpn.lib.model.NotificationId
import com.warrenbrowse.vpn.lib.model.NotificationUpdate
import com.warrenbrowse.vpn.lib.model.StrikeNotice
import com.warrenbrowse.vpn.lib.pushnotification.NotificationProvider
import com.warrenbrowse.vpn.lib.repository.AccountStrikeAlerts
import kotlinx.coroutines.channels.BufferOverflow
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.SharedFlow

/**
 * One system notification per port-forward strike (warren-core doc 105), each
 * under its own id so a second strike does not replace the first: three of
 * them revoke the account, and each one is a warning the reader must see.
 *
 * Unlike the tunnel and forum slots this is a stream of events rather than a
 * state, so it is not debounced: two strikes handed over by one poll are two
 * notifications, not the last of them.
 */
class AccountStrikeNotificationProvider(private val channelId: NotificationChannelId) :
    NotificationProvider<Notification.AccountStrike>, AccountStrikeAlerts {

    private val posted = mutableSetOf<NotificationId>()

    private val _notifications =
        MutableSharedFlow<NotificationUpdate<Notification.AccountStrike>>(
            extraBufferCapacity = BUFFER,
            onBufferOverflow = BufferOverflow.DROP_OLDEST,
        )
    override val notifications: SharedFlow<NotificationUpdate<Notification.AccountStrike>> =
        _notifications

    override val debounced: Boolean = false

    override fun announce(notice: StrikeNotice) {
        val id = idOf(notice)
        synchronized(posted) { posted += id }
        _notifications.tryEmit(
            NotificationUpdate.Notify(id, Notification.AccountStrike(channelId, notice))
        )
    }

    override fun clear() {
        val ids = synchronized(posted) { posted.toList().also { posted.clear() } }
        ids.forEach { _notifications.tryEmit(NotificationUpdate.Cancel(it)) }
    }

    /**
     * An id per strike, from its case reference, clear of the tunnel (2) and
     * forum (3) slots. A repeat of the same strike replaces its own
     * notification, which is what it should do.
     */
    fun idOf(notice: StrikeNotice): NotificationId =
        NotificationId(ID_BASE + (notice.strike.caseReference.hashCode() and ID_MASK))

    private companion object {
        const val ID_BASE = 0x1000
        const val ID_MASK = 0xFFFF
        const val BUFFER = 16
    }
}
