package com.warrenbrowse.vpn.lib.usecase.inappnotification

import com.warrenbrowse.vpn.lib.model.InAppNotification
import com.warrenbrowse.vpn.lib.repository.WarrenHostOfflineProvider
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map

// Immediate "no internet" banner, shown in every tunnel state while the
// device is offline (desktop WarrenHostOfflineNotificationProvider parity).
// The provider's flow is already debounced on the rising edge, so routine
// network handovers never flash it; it drops by itself on the online edge.
class HostOfflineNotificationUseCase(
    private val hostOfflineProvider: WarrenHostOfflineProvider,
) : InAppNotificationUseCase {
    override operator fun invoke(): Flow<InAppNotification?> =
        hostOfflineProvider.hostOffline
            .map { offline -> if (offline) InAppNotification.HostOffline else null }
            .distinctUntilChanged()
}
