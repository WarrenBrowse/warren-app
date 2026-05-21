package com.warrenbrowse.vpn.feature.login.impl.devicelist.navigation

import androidx.navigation3.runtime.EntryProviderScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.login.api.DeviceListNavKey
import com.warrenbrowse.vpn.feature.login.impl.devicelist.DeviceList

fun EntryProviderScope<NavKey2>.deviceListEntry(navigator: Navigator) {
    entry<DeviceListNavKey> { navKey ->
        DeviceList(accountNumber = navKey.accountNumber, navigator = navigator)
    }
}
