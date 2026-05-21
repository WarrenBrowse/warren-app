package com.warrenbrowse.vpn.feature.serveripoverride.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import androidx.navigation3.scene.DialogSceneStrategy
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.serveripoverride.api.ServerIpOverrideInfoNavKey
import com.warrenbrowse.vpn.feature.serveripoverride.impl.info.ServerIpOverridesInfo

fun EntryProviderScope<NavKey2>.serverIpOverrideInfoEntry(navigator: Navigator) {
    entry<ServerIpOverrideInfoNavKey>(metadata = DialogSceneStrategy.dialog()) {
        ServerIpOverridesInfo(navigator = navigator)
    }
}
