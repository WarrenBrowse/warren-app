package com.warrenbrowse.vpn.feature.appinfo.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.core.animation.slideInHorizontalTransition
import com.warrenbrowse.vpn.core.scene.ListDetailSceneStrategy
import com.warrenbrowse.vpn.feature.appinfo.api.AppInfoNavKey
import com.warrenbrowse.vpn.feature.appinfo.api.LicensesNavKey
import com.warrenbrowse.vpn.feature.appinfo.impl.AppInfo
import com.warrenbrowse.vpn.feature.appinfo.impl.licenses.Licenses

internal fun EntryProviderScope<NavKey2>.appInfoEntry(navigator: Navigator) {
    entry<AppInfoNavKey>(
        metadata = ListDetailSceneStrategy.detailPane() + slideInHorizontalTransition()
    ) {
        AppInfo(navigator = navigator)
    }
    entry<LicensesNavKey>(metadata = slideInHorizontalTransition()) { navKey ->
        Licenses(navArgs = navKey, navigator = navigator)
    }
}
