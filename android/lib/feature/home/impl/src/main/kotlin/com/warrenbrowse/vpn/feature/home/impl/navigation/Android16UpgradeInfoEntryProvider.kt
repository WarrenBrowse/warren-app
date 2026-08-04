package com.warrenbrowse.vpn.feature.home.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import androidx.navigation3.scene.DialogSceneStrategy
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.home.api.Android16UpgradeInfoNavKey
import com.warrenbrowse.vpn.feature.home.impl.connect.Android16UpgradeWarningInfo

internal fun EntryProviderScope<NavKey2>.android16UpgradeInfoEntry(navigator: Navigator) {
    entry<Android16UpgradeInfoNavKey>(metadata = DialogSceneStrategy.dialog()) {
        Android16UpgradeWarningInfo(navigator = navigator)
    }
}
