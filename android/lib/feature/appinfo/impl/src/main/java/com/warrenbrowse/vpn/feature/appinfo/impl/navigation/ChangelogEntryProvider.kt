package com.warrenbrowse.vpn.feature.appinfo.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.core.animation.slideInHorizontalTransition
import com.warrenbrowse.vpn.feature.appinfo.api.ChangelogNavKey
import com.warrenbrowse.vpn.feature.appinfo.impl.changelog.Changelog

fun EntryProviderScope<NavKey2>.changelogEntry(navigator: Navigator) {
    entry<ChangelogNavKey>(metadata = slideInHorizontalTransition()) { navKey ->
        Changelog(navArgs = navKey, navigator = navigator)
    }

    appInfoEntry(navigator)
}
