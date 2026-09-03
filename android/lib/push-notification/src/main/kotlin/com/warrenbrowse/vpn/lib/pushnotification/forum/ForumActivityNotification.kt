package com.warrenbrowse.vpn.lib.pushnotification.forum

import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import androidx.core.app.NotificationCompat
import com.warrenbrowse.vpn.lib.common.constant.KEY_OPEN_FORUM_ACTIVITY
import com.warrenbrowse.vpn.lib.common.constant.MAIN_ACTIVITY_CLASS
import com.warrenbrowse.vpn.lib.common.util.getSupportedPendingIntentFlags
import com.warrenbrowse.vpn.lib.model.Notification
import com.warrenbrowse.vpn.lib.model.forum.ForumActivityWording
import com.warrenbrowse.vpn.lib.model.forum.forumActivityWording
import com.warrenbrowse.vpn.lib.ui.resource.R

/**
 * The desktop forum-activity banner as an Android notification: the count in
 * the desktop's wording, a tap opening the activity panel. Secret on the lock
 * screen like the tunnel notification: a forum name is nobody else's business.
 */
fun Notification.Forum.toNotification(context: Context): android.app.Notification =
    NotificationCompat.Builder(context, channelId.value)
        .setContentIntent(contentIntent(context))
        .setContentTitle(forumActivityText(context, unread))
        .setSmallIcon(R.drawable.small_logo_white)
        .setAutoCancel(true)
        .setOnlyAlertOnce(true)
        .setVisibility(NotificationCompat.VISIBILITY_SECRET)
        .build()

/** The desktop `ForumActivityNotificationProvider` wording, per count. */
fun forumActivityText(context: Context, unread: Int): String =
    when (val wording = forumActivityWording(unread)) {
        ForumActivityWording.Single -> context.getString(R.string.forum_activity_notification_one)
        is ForumActivityWording.Several ->
            context.resources.getQuantityString(
                R.plurals.forum_activity_notification_several,
                wording.count,
                wording.count,
            )
        is ForumActivityWording.MoreThan ->
            context.getString(R.string.forum_activity_notification_more_than, wording.count)
    }

private fun contentIntent(context: Context): PendingIntent {
    val intent =
        Intent().apply {
            setClassName(context.packageName, MAIN_ACTIVITY_CLASS)
            flags = Intent.FLAG_ACTIVITY_CLEAR_TOP or Intent.FLAG_ACTIVITY_SINGLE_TOP
            action = KEY_OPEN_FORUM_ACTIVITY
        }
    // Its own request code: the tunnel notification's intent (code 1) is a
    // plain launch, and reusing its code would hand this tap that intent.
    return PendingIntent.getActivity(context, 2, intent, getSupportedPendingIntentFlags())
}
