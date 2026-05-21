package com.warrenbrowse.vpn.feature.managedevices.impl

import com.warrenbrowse.vpn.lib.model.Device

data class ManageDevicesUiState(val devices: List<ManageDevicesItemUiState>)

data class ManageDevicesItemUiState(
    val device: Device,
    val isLoading: Boolean,
    val isCurrentDevice: Boolean,
)
