package com.warrenbrowse.vpn.lib.usecase.inappnotification

import kotlinx.coroutines.flow.Flow
import com.warrenbrowse.vpn.lib.model.InAppNotification

interface InAppNotificationUseCase {
    operator fun invoke(): Flow<InAppNotification?>
}
