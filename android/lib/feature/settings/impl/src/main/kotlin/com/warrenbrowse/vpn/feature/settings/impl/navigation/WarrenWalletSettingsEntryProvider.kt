package com.warrenbrowse.vpn.feature.settings.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.core.animation.slideInHorizontalTransition
import com.warrenbrowse.vpn.core.scene.ListDetailSceneStrategy
import com.warrenbrowse.vpn.feature.settings.api.WarrenWalletSettingsNavKey
import com.warrenbrowse.vpn.feature.settings.impl.WarrenWalletSettings

fun EntryProviderScope<NavKey2>.walletSettingsEntry(navigator: Navigator) {
    entry<WarrenWalletSettingsNavKey>(
        metadata = ListDetailSceneStrategy.detailPane() + slideInHorizontalTransition()
    ) {
        WarrenWalletSettings(navigator = navigator)
    }
}
