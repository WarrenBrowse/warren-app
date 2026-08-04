package com.warrenbrowse.vpn.feature.splittunneling.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.core.animation.slideInHorizontalTransition
import com.warrenbrowse.vpn.core.scene.ListDetailSceneStrategy
import com.warrenbrowse.vpn.feature.splittunneling.api.SearchSplitTunnelingNavKey
import com.warrenbrowse.vpn.feature.splittunneling.impl.search.SearchSplitTunnelingScreen

fun EntryProviderScope<NavKey2>.searchSplitTunnelingEntry(navigator: Navigator) {
    entry<SearchSplitTunnelingNavKey>(
        metadata = ListDetailSceneStrategy.detailPane() + slideInHorizontalTransition()
    ) { _ ->
        SearchSplitTunnelingScreen(navigator = navigator)
    }
}
