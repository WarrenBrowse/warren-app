package com.warrenbrowse.vpn.feature.anticensorship.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import androidx.navigation3.scene.DialogSceneStrategy
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.anticensorship.api.CustomPortNavKey
import com.warrenbrowse.vpn.feature.anticensorship.impl.customport.CustomPort

fun EntryProviderScope<NavKey2>.customPortEntry(navigator: Navigator) {
    entry<CustomPortNavKey>(metadata = DialogSceneStrategy.dialog()) { navKey ->
        CustomPort(navArg = navKey, navigator = navigator)
    }
}
