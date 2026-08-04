package com.warrenbrowse.vpn.feature.settings.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.core.animation.slideInHorizontalTransition
import com.warrenbrowse.vpn.core.scene.ListDetailSceneStrategy
import com.warrenbrowse.vpn.feature.settings.api.WarrenPortForwardingSettingsNavKey
import com.warrenbrowse.vpn.feature.settings.impl.WarrenPortForwardingSettings

fun EntryProviderScope<NavKey2>.warrenPortForwardingSettingsEntry(navigator: Navigator) {
    entry<WarrenPortForwardingSettingsNavKey>(
        metadata = ListDetailSceneStrategy.detailPane() + slideInHorizontalTransition()
    ) {
        WarrenPortForwardingSettings(navigator = navigator)
    }
}
