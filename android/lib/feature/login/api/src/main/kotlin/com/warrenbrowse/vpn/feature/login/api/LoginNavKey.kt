package com.warrenbrowse.vpn.feature.login.api

import kotlinx.parcelize.Parcelize
import com.warrenbrowse.vpn.core.NavKey2

@Parcelize data class LoginNavKey(val accountNumber: String? = null) : NavKey2
