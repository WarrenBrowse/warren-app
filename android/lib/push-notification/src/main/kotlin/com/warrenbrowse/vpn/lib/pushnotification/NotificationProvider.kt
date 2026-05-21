package com.warrenbrowse.vpn.lib.pushnotification

import kotlinx.coroutines.flow.Flow
import com.warrenbrowse.vpn.lib.model.NotificationUpdate

interface NotificationProvider<D> {
    val notifications: Flow<NotificationUpdate<D>>
}
