package com.warrenbrowse.vpn.feature.settings.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.core.animation.slideInHorizontalTransition
import com.warrenbrowse.vpn.core.scene.ListDetailSceneStrategy
import com.warrenbrowse.vpn.feature.settings.api.WarrenMultihopSettingsNavKey
import com.warrenbrowse.vpn.feature.settings.impl.WarrenMultihopSettings

fun EntryProviderScope<NavKey2>.warrenMultihopSettingsEntry(navigator: Navigator) {
    entry<WarrenMultihopSettingsNavKey>(
        metadata = ListDetailSceneStrategy.detailPane() + slideInHorizontalTransition()
    ) {
        WarrenMultihopSettings(navigator = navigator)
    }
}
