package com.warrenbrowse.vpn.feature.home.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.home.api.DeviceRevokedNavKey
import com.warrenbrowse.vpn.feature.home.impl.devicerevoked.DeviceRevoked

internal fun EntryProviderScope<NavKey2>.deviceRevokedEntry(navigator: Navigator) {
    entry<DeviceRevokedNavKey> { DeviceRevoked(navigator = navigator) }
}
