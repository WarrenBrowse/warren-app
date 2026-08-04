package com.warrenbrowse.vpn.feature.settings.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.core.animation.slideUpModalTransition
import com.warrenbrowse.vpn.core.scene.ListDetailSceneStrategy
import com.warrenbrowse.vpn.feature.settings.api.SettingsNavKey
import com.warrenbrowse.vpn.feature.settings.impl.Settings

fun EntryProviderScope<NavKey2>.settingsEntry(navigator: Navigator) {
    // Settings is raised over the main view and dropped back down, matching the desktop
    // header button which pushes it with the show transition.
    entry<SettingsNavKey>(
        metadata = ListDetailSceneStrategy.listPane() + slideUpModalTransition()
    ) {
        Settings(navigator = navigator)
    }
}
