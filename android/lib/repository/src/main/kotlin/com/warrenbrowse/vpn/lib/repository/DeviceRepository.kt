package com.warrenbrowse.vpn.lib.repository

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import com.warrenbrowse.vpn.lib.model.DeviceState

// D.4 step 58: DeviceRepository stripped of ManagementService dependency. The
// Mullvad daemon's `deviceState` channel is dead on Warren — `deviceState`
// permanently emits null and `updateDevice()` is a no-op. ConnectViewModel +
// DeviceRevokedViewModel still inject this for compile-shim ; the dead-daemon
// path will be removed entirely once those VMs are rewired to
// `WarrenTunnelStateProvider` (D.4 step 67+).
@Suppress("UNUSED_PARAMETER", "unused")
class DeviceRepository(@Suppress("UnusedPrivateMember") managementService: Any? = null) {
    val deviceState: StateFlow<DeviceState?> = MutableStateFlow(null)

    suspend fun updateDevice() {
        // no-op
    }
}
