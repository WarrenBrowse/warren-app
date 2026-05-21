package com.warrenbrowse.vpn.feature.home.impl.welcome

import com.warrenbrowse.vpn.lib.model.AccountNumber
import com.warrenbrowse.vpn.lib.model.TunnelState

data class WelcomeUiState(
    val tunnelState: TunnelState,
    val accountNumber: AccountNumber?,
    val deviceName: String?,
    val showSitePayment: Boolean,
    val verificationPending: Boolean,
)
