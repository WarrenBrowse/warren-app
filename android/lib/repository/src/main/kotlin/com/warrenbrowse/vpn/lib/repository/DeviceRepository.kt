package com.warrenbrowse.vpn.lib.repository

import co.touchlab.kermit.Logger
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.stateIn
import com.warrenbrowse.vpn.lib.grpc.ManagementService
import com.warrenbrowse.vpn.lib.model.DeviceState

// D.4 step 51: DeviceRepository slimmed to {deviceState, updateDevice} — drop
// `removeDevice` + `deviceList` (consumers were the deleted ManageDevices /
// DeviceList screens).
class DeviceRepository(
    private val managementService: ManagementService,
    dispatcher: CoroutineDispatcher = Dispatchers.IO,
) {
    val deviceState: StateFlow<DeviceState?> =
        managementService.deviceState.stateIn(
            CoroutineScope(dispatcher),
            SharingStarted.Eagerly,
            null,
        )

    suspend fun updateDevice() {
        Logger.i("Update device")
        managementService.updateDevice()
    }
}
