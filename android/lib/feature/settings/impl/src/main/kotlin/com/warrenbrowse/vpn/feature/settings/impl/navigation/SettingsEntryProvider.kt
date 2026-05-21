package com.warrenbrowse.vpn.feature.settings.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.core.animation.topLevelTransition
import com.warrenbrowse.vpn.core.scene.ListDetailSceneStrategy
import com.warrenbrowse.vpn.feature.settings.api.SettingsNavKey
import com.warrenbrowse.vpn.feature.settings.impl.Settings

fun EntryProviderScope<NavKey2>.settingsEntry(navigator: Navigator) {
    entry<SettingsNavKey>(metadata = ListDetailSceneStrategy.listPane() + topLevelTransition()) {
        Settings(navigator = navigator)
    }
}
