package com.warrenbrowse.vpn.feature.login.impl.devicelist

import com.warrenbrowse.vpn.lib.model.Device
import com.warrenbrowse.vpn.lib.model.GetDeviceListError

sealed interface DeviceListUiState {
    data object Loading : DeviceListUiState

    data class Error(val error: GetDeviceListError) : DeviceListUiState

    data class Content(val devices: List<DeviceItemUiState>) : DeviceListUiState {
        val hasTooManyDevices = devices.size >= MAXIMUM_DEVICES
    }

    companion object {
        val INITIAL: DeviceListUiState = Loading
    }
}

data class DeviceItemUiState(val device: Device, val isLoading: Boolean)

private const val MAXIMUM_DEVICES = 5
