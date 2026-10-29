package com.warrenbrowse.vpn.feature.managedevices.impl

import com.warrenbrowse.vpn.lib.model.AccountDevice
import java.time.LocalDate

data class ManageDevicesUiState(
    /** This device first, then the others by last activity. */
    val devices: List<ManageDevicesItemUiState>,
    /** The day of the older-client row; no row when `null`. */
    val olderClientSeenOn: LocalDate?,
    /** The UTC day the registry's days are read against ("Active today"). */
    val today: LocalDate,
)

data class ManageDevicesItemUiState(
    val device: AccountDevice,
    /** A request about this device is in flight: its row waits. */
    val isLoading: Boolean,
)
