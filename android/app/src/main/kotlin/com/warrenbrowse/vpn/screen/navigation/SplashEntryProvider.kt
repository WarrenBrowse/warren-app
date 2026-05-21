package com.warrenbrowse.vpn.screen.navigation

import androidx.navigation3.runtime.EntryProviderScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.screen.splash.Splash

fun EntryProviderScope<NavKey2>.splashEntry(navigator: Navigator) {
    entry<SplashNavKey> { Splash(navigator = navigator) }
}
