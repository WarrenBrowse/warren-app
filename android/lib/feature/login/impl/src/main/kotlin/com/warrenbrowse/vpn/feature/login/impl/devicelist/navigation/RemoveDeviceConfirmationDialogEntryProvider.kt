package com.warrenbrowse.vpn.feature.login.impl.devicelist.navigation

import androidx.navigation3.runtime.EntryProviderScope
import androidx.navigation3.scene.DialogSceneStrategy
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.login.api.RemoveDeviceNavKey
import com.warrenbrowse.vpn.feature.login.impl.devicelist.RemoveDeviceConfirmation

fun EntryProviderScope<NavKey2>.removeDeviceConfirmationDialogEntry(navigator: Navigator) {
    entry<RemoveDeviceNavKey>(metadata = DialogSceneStrategy.dialog()) { navKey ->
        RemoveDeviceConfirmation(navigator = navigator, device = navKey.device)
    }
}
