package com.warrenbrowse.vpn.lib.usecase.inappnotification

import com.warrenbrowse.vpn.lib.model.InAppNotification
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map

/**
 * Advises turning on Always-on VPN with "Block connections without VPN" while the service last
 * found it off, until the user puts the advice away.
 */
class AlwaysOnVpnAdviceNotificationUseCase(private val showAdvice: () -> Flow<Boolean>) :
    InAppNotificationUseCase {
    override operator fun invoke(): Flow<InAppNotification?> =
        showAdvice()
            .map { show -> if (show) InAppNotification.AlwaysOnVpnAdvice else null }
            .distinctUntilChanged()
}
