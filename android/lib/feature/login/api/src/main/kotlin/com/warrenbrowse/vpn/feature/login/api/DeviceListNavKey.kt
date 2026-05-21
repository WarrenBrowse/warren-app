package com.warrenbrowse.vpn.feature.login.api

import kotlinx.parcelize.Parcelize
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.lib.model.AccountNumber

@Parcelize data class DeviceListNavKey(val accountNumber: AccountNumber) : NavKey2
