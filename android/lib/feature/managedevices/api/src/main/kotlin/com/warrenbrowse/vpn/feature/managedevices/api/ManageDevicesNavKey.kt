package com.warrenbrowse.vpn.feature.managedevices.api

import kotlinx.parcelize.Parcelize
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.lib.model.AccountNumber

@Parcelize data class ManageDevicesNavKey(val accountNumber: AccountNumber) : NavKey2
