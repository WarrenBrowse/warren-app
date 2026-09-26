package com.warrenbrowse.vpn.feature.settings.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.core.animation.slideInHorizontalTransition
import com.warrenbrowse.vpn.core.scene.ListDetailSceneStrategy
import com.warrenbrowse.vpn.feature.settings.api.WarrenNetworkNavKey
import com.warrenbrowse.vpn.feature.settings.impl.WarrenNetwork

fun EntryProviderScope<NavKey2>.warrenNetworkEntry(navigator: Navigator) {
    entry<WarrenNetworkNavKey>(
        metadata = ListDetailSceneStrategy.detailPane() + slideInHorizontalTransition()
    ) {
        WarrenNetwork(navigator = navigator)
    }
}
