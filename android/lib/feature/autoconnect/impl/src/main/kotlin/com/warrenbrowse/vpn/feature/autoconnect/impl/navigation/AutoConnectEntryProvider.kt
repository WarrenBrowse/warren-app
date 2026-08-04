package com.warrenbrowse.vpn.feature.autoconnect.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.core.animation.slideInHorizontalTransition
import com.warrenbrowse.vpn.core.scene.ListDetailSceneStrategy
import com.warrenbrowse.vpn.feature.autoconnect.api.AutoConnectNavKey
import com.warrenbrowse.vpn.feature.autoconnect.impl.AutoConnectAndLockdownMode

fun EntryProviderScope<NavKey2>.autoConnectEntry(navigator: Navigator) {
    entry<AutoConnectNavKey>(
        metadata = ListDetailSceneStrategy.detailPane() + slideInHorizontalTransition()
    ) {
        AutoConnectAndLockdownMode(navigator = navigator)
    }
}
