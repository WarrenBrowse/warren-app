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
                // D.4 step 28: fade in when arriving from the wallet
                // onboarding (was Mullvad LoginNavKey).
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
    // D.4 step 18: outOfTimeEntry + welcomeEntry removed - both screens are
    // Mullvad account-driven (out-of-time = subscription expired ;
    // welcome = "your new account number is X"). Warren uses BIP39 wallet
    // identity ; neither screen has a Warren equivalent so the routes
    // are gone. Splash + login-screen call sites that referenced these
    // NavKeys were rewired to ConnectNavKey directly.
}
