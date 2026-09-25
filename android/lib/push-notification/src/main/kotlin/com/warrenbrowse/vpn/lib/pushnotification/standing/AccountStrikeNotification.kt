package com.warrenbrowse.vpn.lib.pushnotification.standing

import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import androidx.core.app.NotificationCompat
import com.warrenbrowse.vpn.lib.common.constant.MAIN_ACTIVITY_CLASS
import com.warrenbrowse.vpn.lib.common.util.AccountStandingText
import com.warrenbrowse.vpn.lib.common.util.getSupportedPendingIntentFlags
import com.warrenbrowse.vpn.lib.model.Notification
import com.warrenbrowse.vpn.lib.ui.resource.R

/**
 * The desktop strike notification as an Android one: "Warning 1 of 3: port N
 * was closed on DAY after an abuse report", with the case reference to quote
 * when contesting it. Secret on the lock screen, like the tunnel notification:
 * the port and the case are the account's business only.
 */
fun Notification.AccountStrike.toNotification(context: Context): android.app.Notification {
    val locale = context.resources.configuration.locales[0]
    val warning = AccountStandingText.warning(context, notice, locale)
    val text = warning + " " + AccountStandingText.caseReference(context, notice.strike)
    return NotificationCompat.Builder(context, channelId.value)
        .setContentIntent(contentIntent(context))
        .setContentTitle(context.getString(R.string.account_strike_notification_title))
        .setContentText(warning)
        .setStyle(NotificationCompat.BigTextStyle().bigText(text))
        .setSmallIcon(R.drawable.small_logo_white)
        .setAutoCancel(true)
        .setVisibility(NotificationCompat.VISIBILITY_SECRET)
        .build()
}

private fun contentIntent(context: Context): PendingIntent {
    val intent =
        Intent().apply {
            setClassName(context.packageName, MAIN_ACTIVITY_CLASS)
            flags = Intent.FLAG_ACTIVITY_CLEAR_TOP or Intent.FLAG_ACTIVITY_SINGLE_TOP
        }
    return PendingIntent.getActivity(
        context,
        REQUEST_CODE,
        intent,
        getSupportedPendingIntentFlags(),
    )
}

// Its own request code: the tunnel notification uses 1 and the forum one 2.
private const val REQUEST_CODE = 3
