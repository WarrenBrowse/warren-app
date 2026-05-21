package com.warrenbrowse.vpn.feature.anticensorship.impl.customport

import com.warrenbrowse.vpn.lib.model.ParsePortError
import com.warrenbrowse.vpn.lib.model.PortRange

data class CustomPortDialogUiState(
    val portInput: String,
    val portInputError: ParsePortError?,
    val allowedPortRanges: List<PortRange>,
    val recommendedPortRanges: List<PortRange>,
    val showResetToDefault: Boolean,
)
