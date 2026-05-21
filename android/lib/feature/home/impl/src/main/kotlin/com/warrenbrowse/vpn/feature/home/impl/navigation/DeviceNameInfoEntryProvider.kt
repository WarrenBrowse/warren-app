package com.warrenbrowse.vpn.feature.home.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import androidx.navigation3.scene.DialogSceneStrategy
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.home.api.DeviceNameInfoNavKey
import com.warrenbrowse.vpn.feature.home.impl.welcome.DeviceNameInfo

internal fun EntryProviderScope<NavKey2>.deviceNameInfoEntry(navigator: Navigator) {
    entry<DeviceNameInfoNavKey>(metadata = DialogSceneStrategy.dialog()) {
        DeviceNameInfo(navigator = navigator)
    }
}
