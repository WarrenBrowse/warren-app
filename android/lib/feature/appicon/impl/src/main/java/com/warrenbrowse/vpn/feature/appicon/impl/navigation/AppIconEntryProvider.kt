package com.warrenbrowse.vpn.feature.appicon.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.core.animation.slideInHorizontalTransition
import com.warrenbrowse.vpn.feature.appicon.api.AppIconNavKey
import com.warrenbrowse.vpn.feature.appicon.impl.AppIcon

fun EntryProviderScope<NavKey2>.appIconEntry(navigator: Navigator) {
    entry<AppIconNavKey>(metadata = slideInHorizontalTransition()) {
        AppIcon(navigator = navigator)
    }
}
