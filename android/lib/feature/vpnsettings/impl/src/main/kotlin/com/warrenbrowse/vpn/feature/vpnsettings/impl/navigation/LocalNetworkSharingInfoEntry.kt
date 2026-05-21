package com.warrenbrowse.vpn.feature.vpnsettings.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import androidx.navigation3.scene.DialogSceneStrategy
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.vpnsettings.api.LocalNetworkSharingInfoNavKey
import com.warrenbrowse.vpn.feature.vpnsettings.impl.info.LocalNetworkSharingInfo

internal fun EntryProviderScope<NavKey2>.localNetworkSharingInfoEntry(navigator: Navigator) {
    entry<LocalNetworkSharingInfoNavKey>(metadata = DialogSceneStrategy.dialog()) {
        LocalNetworkSharingInfo(navigator = navigator)
    }
}
