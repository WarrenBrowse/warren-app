package com.warrenbrowse.vpn.feature.login.api

import kotlinx.parcelize.Parcelize
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.NavResult
import com.warrenbrowse.vpn.lib.model.Device
import com.warrenbrowse.vpn.lib.model.DeviceId

@Parcelize data class RemoveDeviceNavKey(val device: Device) : NavKey2

@Parcelize data class RemoveDeviceConfirmationDialogResult(val device: DeviceId) : NavResult
