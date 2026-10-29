package com.warrenbrowse.vpn.feature.managedevices.api

import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.NavResult
import kotlinx.parcelize.Parcelize

/** Confirms disconnecting another device of the account, named [name] as the list shows it. */
@Parcelize
data class ManageDevicesRemoveConfirmationNavKey(val deviceId: String, val name: String) : NavKey2

@Parcelize data class ManageDevicesRemoveConfirmationNavResult(val deviceId: String) : NavResult
