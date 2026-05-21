package com.warrenbrowse.vpn.feature.managedevices.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import androidx.navigation3.scene.DialogSceneStrategy
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.managedevices.api.ManageDevicesRemoveConfirmationNavKey
import com.warrenbrowse.vpn.feature.managedevices.impl.confirmation.ManageDevicesRemoveConfirmation

internal fun EntryProviderScope<NavKey2>.manageDevicesRemoveConfirmationEntry(
    navigator: Navigator
) {
    entry<ManageDevicesRemoveConfirmationNavKey>(metadata = DialogSceneStrategy.dialog()) { navKey
        ->
        ManageDevicesRemoveConfirmation(navigator = navigator, device = navKey.device)
    }
}
