package com.warrenbrowse.vpn.feature.vpnsettings.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import androidx.navigation3.scene.DialogSceneStrategy
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.vpnsettings.api.ContentBlockersInfoNavKey
import com.warrenbrowse.vpn.feature.vpnsettings.impl.info.ContentBlockersInfo

internal fun EntryProviderScope<NavKey2>.contentBlockersInfoEntry(navigator: Navigator) {
    entry<ContentBlockersInfoNavKey>(metadata = DialogSceneStrategy.dialog()) {
        ContentBlockersInfo(navigator = navigator)
    }
}
