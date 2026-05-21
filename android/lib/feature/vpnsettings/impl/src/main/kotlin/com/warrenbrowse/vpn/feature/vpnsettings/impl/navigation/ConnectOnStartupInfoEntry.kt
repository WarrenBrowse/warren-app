package com.warrenbrowse.vpn.feature.vpnsettings.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import androidx.navigation3.scene.DialogSceneStrategy
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.vpnsettings.api.ConnectOnStartupInfoNavKey
import com.warrenbrowse.vpn.feature.vpnsettings.impl.info.ConnectOnStartupInfo

internal fun EntryProviderScope<NavKey2>.connectOnStartupInfoEntry(navigator: Navigator) {
    entry<ConnectOnStartupInfoNavKey>(metadata = DialogSceneStrategy.dialog()) {
        ConnectOnStartupInfo(navigator = navigator)
    }
}
