package com.warrenbrowse.vpn.feature.login.impl.apiunreachable

import com.warrenbrowse.vpn.feature.login.api.LoginAction

data class ApiUnreachableUiState(
    val showEnableAllAccessMethodsButton: Boolean,
    val noEmailAppAvailable: Boolean,
    val loginAction: LoginAction,
)
