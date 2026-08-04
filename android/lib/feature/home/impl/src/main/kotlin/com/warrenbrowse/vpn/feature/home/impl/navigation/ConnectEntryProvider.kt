package com.warrenbrowse.vpn.feature.home.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import androidx.navigation3.ui.LocalNavAnimatedContentScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.core.animation.homeTransition
import com.warrenbrowse.vpn.feature.home.api.ConnectNavKey
import com.warrenbrowse.vpn.feature.home.impl.connect.Connect
import com.warrenbrowse.vpn.feature.login.api.WarrenWalletNavKey

fun EntryProviderScope<NavKey2>.homeEntry(navigator: Navigator) {
    entry<ConnectNavKey>(
        metadata =
            homeTransition {
                // Fade in when arriving from the wallet onboarding.
                navigator.previousBackStack.last() is WarrenWalletNavKey
            }
    ) {
        Connect(
            navigator = navigator,
            animatedVisibilityScope = LocalNavAnimatedContentScope.current,
        )
    }

    android16UpgradeInfoEntry(navigator)
    deviceRevokedEntry(navigator)
}
