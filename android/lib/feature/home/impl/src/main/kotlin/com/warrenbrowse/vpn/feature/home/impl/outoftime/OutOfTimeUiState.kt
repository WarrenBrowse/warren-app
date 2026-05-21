package com.warrenbrowse.vpn.feature.home.impl.outoftime

import com.warrenbrowse.vpn.lib.model.TunnelState

data class OutOfTimeUiState(
    val tunnelState: TunnelState = TunnelState.Disconnected(),
    val deviceName: String = "",
    val showSitePayment: Boolean = false,
    val verificationPending: Boolean = false,
)
