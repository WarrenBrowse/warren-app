package com.warrenbrowse.vpn.feature.home.impl.connect.notificationbanner

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import com.warrenbrowse.vpn.lib.model.InAppNotification
import com.warrenbrowse.vpn.lib.usecase.inappnotification.InAppNotificationUseCase

class InAppNotificationController(
    inAppNotificationUseCases: List<InAppNotificationUseCase>,
    scope: CoroutineScope,
) {

    val notifications =
        combine(inAppNotificationUseCases.map { it.invoke() }) {
                notifications: Array<InAppNotification?> ->
                notifications.filterNotNull()
            }
            .map {
                it.sortedWith(
                    compareBy(
                        { notification -> -notification.priority },
                        { notification -> notification.statusLevel.ordinal },
                    )
                )
            }
            .stateIn(scope, SharingStarted.Eagerly, emptyList())
}
