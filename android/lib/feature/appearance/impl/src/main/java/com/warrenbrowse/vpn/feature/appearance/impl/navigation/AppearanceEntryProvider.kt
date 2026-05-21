package com.warrenbrowse.vpn.feature.appearance.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.core.animation.slideInHorizontalTransition
import com.warrenbrowse.vpn.core.scene.ListDetailSceneStrategy
import com.warrenbrowse.vpn.feature.appearance.api.AppearanceNavKey
import com.warrenbrowse.vpn.feature.appearance.impl.Appearance

fun EntryProviderScope<NavKey2>.appearanceEntry(navigator: Navigator) {
    entry<AppearanceNavKey>(
        metadata = ListDetailSceneStrategy.detailPane() + slideInHorizontalTransition()
    ) {
        Appearance(navigator = navigator)
    }
}
