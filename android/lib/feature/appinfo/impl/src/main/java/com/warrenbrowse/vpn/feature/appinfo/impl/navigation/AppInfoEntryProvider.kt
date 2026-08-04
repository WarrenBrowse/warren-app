package com.warrenbrowse.vpn.feature.appinfo.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.core.animation.slideInHorizontalTransition
import com.warrenbrowse.vpn.core.scene.ListDetailSceneStrategy
import com.warrenbrowse.vpn.feature.appinfo.api.AppInfoNavKey
import com.warrenbrowse.vpn.feature.appinfo.impl.AppInfo

internal fun EntryProviderScope<NavKey2>.appInfoEntry(navigator: Navigator) {
    entry<AppInfoNavKey>(
        metadata = ListDetailSceneStrategy.detailPane() + slideInHorizontalTransition()
    ) {
        AppInfo(navigator = navigator)
    }
}
