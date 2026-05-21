package com.warrenbrowse.vpn.feature.serveripoverride.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import androidx.navigation3.scene.DialogSceneStrategy
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.serveripoverride.api.ResetServerIpOverrideConfirmationNavKey
import com.warrenbrowse.vpn.feature.serveripoverride.impl.reset.ResetServerIpOverridesConfirmation

internal fun EntryProviderScope<NavKey2>.resetServerIpOverrideConfirmationEntry(
    navigator: Navigator
) {
    entry<ResetServerIpOverrideConfirmationNavKey>(metadata = DialogSceneStrategy.dialog()) {
        ResetServerIpOverridesConfirmation(navigator = navigator)
    }
}
