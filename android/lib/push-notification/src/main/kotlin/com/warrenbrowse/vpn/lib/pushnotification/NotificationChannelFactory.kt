package com.warrenbrowse.vpn.lib.pushnotification

import android.app.NotificationManager
import android.content.res.Resources
import androidx.core.app.NotificationChannelCompat
import androidx.core.app.NotificationManagerCompat
import com.warrenbrowse.vpn.lib.common.R
import com.warrenbrowse.vpn.lib.model.NotificationChannel
import com.warrenbrowse.vpn.lib.model.NotificationChannelId

class NotificationChannelFactory(
    private val notificationManagerCompat: NotificationManagerCompat,
    private val resources: Resources,
    channels: List<NotificationChannel>,
) {
    init {
        channels.forEach { create(it) }
    }

    private fun create(channel: NotificationChannel): NotificationChannelId {
        val androidChannel = channel.toAndroidNotificationChannel()
        notificationManagerCompat.createNotificationChannel(androidChannel)
        return channel.id
    }

    private fun NotificationChannel.toAndroidNotificationChannel(): NotificationChannelCompat =
        when (this) {
            NotificationChannel.TunnelUpdates -> NotificationChannel.TunnelUpdates.toChannel()
            NotificationChannel.ForumActivity -> NotificationChannel.ForumActivity.toChannel()
        }

    // The launcher badge is the Android stand-in for the desktop tray dot: it
    // shows while the notification is up and goes with it.
    private fun NotificationChannel.ForumActivity.toChannel(): NotificationChannelCompat =
        NotificationChannelCompat.Builder(id.value, NotificationManager.IMPORTANCE_LOW)
            .setName(resources.getString(R.string.forum_activity_channel_name))
            .setDescription(resources.getString(R.string.forum_activity_channel_description))
            .setShowBadge(true)
            .setVibrationEnabled(false)
            .build()

    private fun NotificationChannel.TunnelUpdates.toChannel(): NotificationChannelCompat =
        NotificationChannelCompat.Builder(id.value, NotificationManager.IMPORTANCE_LOW)
            .setName(resources.getString(R.string.foreground_notification_channel_name))
            .setDescription(
                resources.getString(R.string.foreground_notification_channel_description)
            )
            .setShowBadge(false)
            .setVibrationEnabled(false)
            .build()
}
