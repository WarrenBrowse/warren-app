package com.warrenbrowse.vpn.feature.managedevices.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.core.animation.slideInHorizontalTransition
import com.warrenbrowse.vpn.core.scene.ListDetailSceneStrategy
import com.warrenbrowse.vpn.feature.managedevices.api.ManageDevicesNavKey
import com.warrenbrowse.vpn.feature.managedevices.impl.ManageDevices

fun EntryProviderScope<NavKey2>.manageDevicesEntry(navigator: Navigator) {
    entry<ManageDevicesNavKey>(
        metadata = ListDetailSceneStrategy.detailPane() + slideInHorizontalTransition()
    ) {
        ManageDevices(navigator = navigator)
    }

    manageDevicesRemoveConfirmationEntry(navigator)
}
