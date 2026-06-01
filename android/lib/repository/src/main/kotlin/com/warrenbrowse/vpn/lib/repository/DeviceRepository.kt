package com.warrenbrowse.vpn.lib.repository

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import com.warrenbrowse.vpn.lib.model.DeviceState

// Warren has no device state: `deviceState` always emits null and
// `updateDevice()` does nothing. ConnectViewModel + DeviceRevokedViewModel
// inject this until they are rewired to `WarrenTunnelStateProvider`.
@Suppress("UNUSED_PARAMETER", "unused")
class DeviceRepository(@Suppress("UnusedPrivateMember") managementService: Any? = null) {
    val deviceState: StateFlow<DeviceState?> = MutableStateFlow(null)

    suspend fun updateDevice() {
        // no-op
    }
}
