package com.warrenbrowse.vpn.lib.pushnotification.forum

import com.warrenbrowse.vpn.lib.model.Notification
import com.warrenbrowse.vpn.lib.model.NotificationChannelId
import com.warrenbrowse.vpn.lib.model.NotificationUpdate
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Test

class ForumActivityNotificationProviderTest {

    private val channel = NotificationChannelId("forum_activity")

    @Test
    fun nothing_is_posted_until_a_rise_is_announced() {
        val provider = ForumActivityNotificationProvider(channel)

        assertEquals(NotificationUpdate.Cancel(provider.notificationId), provider.notifications.value)
    }

    @Test
    fun a_rise_posts_the_count_and_a_later_rise_replaces_it_in_the_same_slot() {
        val provider = ForumActivityNotificationProvider(channel)

        provider.announce(1)
        assertEquals(
            NotificationUpdate.Notify(provider.notificationId, Notification.Forum(channel, 1)),
            provider.notifications.value,
        )

        provider.announce(3)
        assertEquals(
            NotificationUpdate.Notify(provider.notificationId, Notification.Forum(channel, 3)),
            provider.notifications.value,
        )
    }

    @Test
    fun the_slot_comes_down_once_nothing_is_waiting() {
        val provider = ForumActivityNotificationProvider(channel)
        provider.announce(2)

        provider.clear()

        assertEquals(NotificationUpdate.Cancel(provider.notificationId), provider.notifications.value)
    }
}
