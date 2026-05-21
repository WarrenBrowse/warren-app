package com.warrenbrowse.vpn.feature.login.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.core.animation.loginTransition
import com.warrenbrowse.vpn.feature.home.api.ConnectNavKey
import com.warrenbrowse.vpn.feature.login.api.DeviceListNavKey
import com.warrenbrowse.vpn.feature.login.api.LoginNavKey
import com.warrenbrowse.vpn.feature.login.impl.Login

fun EntryProviderScope<NavKey2>.loginEntry(navigator: Navigator) {

    entry<LoginNavKey>(
        metadata =
            loginTransition {
                // Fade out if we are navigating to one of the following
                when (navigator.backStack.dropLast(1).lastOrNull()) {
                    ConnectNavKey,
                    is DeviceListNavKey -> true
                    else -> false
                }
            }
    ) { navKey ->
        Login(navigator = navigator, accountNumber = navKey.accountNumber)
    }

    apiUnreachableEntry(navigator)
    createAccountConfirmationEntry(navigator)
}
