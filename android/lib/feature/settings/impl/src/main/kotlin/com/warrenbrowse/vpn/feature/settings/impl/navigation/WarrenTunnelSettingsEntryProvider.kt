package com.warrenbrowse.vpn.feature.settings.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.core.animation.slideInHorizontalTransition
import com.warrenbrowse.vpn.core.scene.ListDetailSceneStrategy
import com.warrenbrowse.vpn.feature.settings.api.WarrenTunnelSettingsNavKey
import com.warrenbrowse.vpn.feature.settings.impl.WarrenVpnSettings

fun EntryProviderScope<NavKey2>.warrenTunnelSettingsEntry(navigator: Navigator) {
    entry<WarrenTunnelSettingsNavKey>(
        metadata = ListDetailSceneStrategy.detailPane() + slideInHorizontalTransition()
    ) {
        WarrenVpnSettings(navigator = navigator)
    }
}
