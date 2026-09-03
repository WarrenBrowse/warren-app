package com.warrenbrowse.vpn.lib.pushnotification.forum

import com.warrenbrowse.vpn.lib.model.Notification
import com.warrenbrowse.vpn.lib.model.NotificationChannelId
import com.warrenbrowse.vpn.lib.model.NotificationId
import com.warrenbrowse.vpn.lib.model.NotificationUpdate
import com.warrenbrowse.vpn.lib.pushnotification.NotificationProvider
import com.warrenbrowse.vpn.lib.repository.ForumActivityAlerts
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow

/**
 * One notification slot for forum activity: a rise replaces what is posted,
 * and the slot comes down as soon as nothing is waiting (the panel was read,
 * the list marked seen, or the forum's own count fell), so a stale invitation
 * never lingers in the shade.
 */
class ForumActivityNotificationProvider(private val channelId: NotificationChannelId) :
    NotificationProvider<Notification.Forum>, ForumActivityAlerts {
    val notificationId = NotificationId(3)

    private val _notifications =
        MutableStateFlow<NotificationUpdate<Notification.Forum>>(NotificationUpdate.Cancel(notificationId))
    override val notifications: StateFlow<NotificationUpdate<Notification.Forum>> = _notifications

    override fun announce(unread: Int) {
        _notifications.value = NotificationUpdate.Notify(notificationId, Notification.Forum(channelId, unread))
    }

    override fun clear() {
        _notifications.value = NotificationUpdate.Cancel(notificationId)
    }
}
