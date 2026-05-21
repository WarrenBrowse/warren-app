package com.warrenbrowse.vpn.receiver

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import co.touchlab.kermit.Logger
import com.warrenbrowse.vpn.lib.pushnotification.ScheduleNotificationAlarmUseCase
import com.warrenbrowse.vpn.lib.repository.UserPreferencesRepository
import com.warrenbrowse.vpn.receiver.util.goAsync
import org.koin.core.component.KoinComponent
import org.koin.core.component.inject

class ScheduleNotificationBootCompletedReceiver : BroadcastReceiver(), KoinComponent {
    private val userPreferencesRepository by inject<UserPreferencesRepository>()
    private val scheduleNotificationAlarmUseCase by inject<ScheduleNotificationAlarmUseCase>()

    override fun onReceive(context: Context?, intent: Intent?) {
        if (intent?.action == Intent.ACTION_BOOT_COMPLETED) {
            context?.let {
                Logger.d(
                    "Scheduling notification alarm from ScheduleNotificationBootCompletedReceiver"
                )
                goAsync { scheduleAccountExpiryNotification(context) }
            }
        }
    }

    private suspend fun scheduleAccountExpiryNotification(context: Context) {
        val expiry = userPreferencesRepository.accountExpiry() ?: return
        scheduleNotificationAlarmUseCase(expiry, customContext = context)
    }
}
