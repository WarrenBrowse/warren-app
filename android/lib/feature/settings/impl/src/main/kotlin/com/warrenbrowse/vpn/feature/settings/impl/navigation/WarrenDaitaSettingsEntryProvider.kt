package com.warrenbrowse.vpn.feature.settings.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.core.animation.slideInHorizontalTransition
import com.warrenbrowse.vpn.core.scene.ListDetailSceneStrategy
import com.warrenbrowse.vpn.feature.settings.api.WarrenDaitaSettingsNavKey
import com.warrenbrowse.vpn.feature.settings.impl.WarrenDaitaSettings

fun EntryProviderScope<NavKey2>.warrenDaitaSettingsEntry(navigator: Navigator) {
    entry<WarrenDaitaSettingsNavKey>(
        metadata = ListDetailSceneStrategy.detailPane() + slideInHorizontalTransition()
    ) {
        WarrenDaitaSettings(navigator = navigator)
    }
}
