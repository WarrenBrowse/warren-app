package com.warrenbrowse.vpn.feature.apiaccess.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import androidx.navigation3.scene.DialogSceneStrategy
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.apiaccess.api.ApiAccessMethodInfoNavKey
import com.warrenbrowse.vpn.feature.apiaccess.impl.screen.info.ApiAccessMethodInfo

internal fun EntryProviderScope<NavKey2>.apiAccessMethodInfoEntry(navigator: Navigator) {
    entry<ApiAccessMethodInfoNavKey>(metadata = DialogSceneStrategy.dialog()) {
        ApiAccessMethodInfo(navigator = navigator)
    }
}
