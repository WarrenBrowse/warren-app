package com.warrenbrowse.vpn.feature.vpnsettings.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import androidx.navigation3.scene.DialogSceneStrategy
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.vpnsettings.api.Ipv6InfoNavKey
import com.warrenbrowse.vpn.feature.vpnsettings.impl.info.Ipv6Info

internal fun EntryProviderScope<NavKey2>.ipv6InfoEntry(navigator: Navigator) {
    entry<Ipv6InfoNavKey>(metadata = DialogSceneStrategy.dialog()) {
        Ipv6Info(navigator = navigator)
    }
}
