package com.warrenbrowse.vpn.lib.repository

import arrow.core.Either
import co.touchlab.kermit.Logger
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.stateIn
import com.warrenbrowse.vpn.lib.grpc.ManagementService
import com.warrenbrowse.vpn.lib.model.AccountNumber
import com.warrenbrowse.vpn.lib.model.DeleteDeviceError
import com.warrenbrowse.vpn.lib.model.Device
import com.warrenbrowse.vpn.lib.model.DeviceId
import com.warrenbrowse.vpn.lib.model.DeviceState
import com.warrenbrowse.vpn.lib.model.GetDeviceListError

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

    suspend fun removeDevice(
        accountNumber: AccountNumber,
        deviceId: DeviceId,
    ): Either<DeleteDeviceError, Unit> = managementService.removeDevice(accountNumber, deviceId)

    suspend fun deviceList(accountNumber: AccountNumber): Either<GetDeviceListError, List<Device>> =
        managementService.getDeviceList(accountNumber)

    suspend fun updateDevice() {
        Logger.i("Update device")
        managementService.updateDevice()
    }
}
