package com.warrenbrowse.vpn.feature.appinfo.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.core.animation.slideInHorizontalTransition
import com.warrenbrowse.vpn.core.animation.slideUpModalTransition
import com.warrenbrowse.vpn.feature.appinfo.api.ChangelogNavKey
import com.warrenbrowse.vpn.feature.appinfo.impl.changelog.Changelog

fun EntryProviderScope<NavKey2>.changelogEntry(navigator: Navigator) {
    // isModal already swaps the back arrow for a close icon; it drives the axis too, so the
    // screen the user closes is the one that came up from the bottom.
    entry<ChangelogNavKey>(
        metadata = { navKey ->
            if (navKey.isModal) slideUpModalTransition() else slideInHorizontalTransition()
        }
    ) { navKey ->
        Changelog(navArgs = navKey, navigator = navigator)
    }

    appInfoEntry(navigator)
}
