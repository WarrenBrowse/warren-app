package com.warrenbrowse.vpn.feature.vpnsettings.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import androidx.navigation3.scene.DialogSceneStrategy
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.vpnsettings.api.MtuNavKey
import com.warrenbrowse.vpn.feature.vpnsettings.impl.mtu.Mtu

internal fun EntryProviderScope<NavKey2>.mtuEntry(navigator: Navigator) {
    entry<MtuNavKey>(metadata = DialogSceneStrategy.dialog()) { navArgs ->
        Mtu(navArgs = navArgs, navigator = navigator)
    }
}
