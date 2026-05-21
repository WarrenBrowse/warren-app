package com.warrenbrowse.vpn.feature.vpnsettings.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import androidx.navigation3.scene.DialogSceneStrategy
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.vpnsettings.api.CustomDnsInfoNavKey
import com.warrenbrowse.vpn.feature.vpnsettings.impl.info.CustomDnsInfo

internal fun EntryProviderScope<NavKey2>.customDnsInfoEntry(navigator: Navigator) {
    entry<CustomDnsInfoNavKey>(metadata = DialogSceneStrategy.dialog()) {
        CustomDnsInfo(navigator = navigator)
    }
}
