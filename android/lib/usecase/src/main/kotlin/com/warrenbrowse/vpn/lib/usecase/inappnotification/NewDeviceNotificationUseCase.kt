package com.warrenbrowse.vpn.lib.usecase.inappnotification

import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map
import com.warrenbrowse.vpn.lib.model.InAppNotification
import com.warrenbrowse.vpn.lib.repository.DeviceRepository
import com.warrenbrowse.vpn.lib.repository.NewDeviceRepository

class NewDeviceNotificationUseCase(
    private val newDeviceRepository: NewDeviceRepository,
    private val deviceRepository: DeviceRepository,
) : InAppNotificationUseCase {
    override operator fun invoke() =
        combine(
                deviceRepository.deviceState.map { it?.displayName() },
                newDeviceRepository.isNewDevice,
            ) { deviceName, newDeviceCreated ->
                if (newDeviceCreated && deviceName != null) {
                    InAppNotification.NewDevice(deviceName)
                } else null
            }
            .distinctUntilChanged()
}
