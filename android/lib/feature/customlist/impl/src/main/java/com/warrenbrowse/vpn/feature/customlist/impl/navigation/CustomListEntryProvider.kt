package com.warrenbrowse.vpn.feature.customlist.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.core.animation.slideInHorizontalTransition
import com.warrenbrowse.vpn.feature.customlist.api.CustomListNavKey
import com.warrenbrowse.vpn.feature.customlist.impl.screen.lists.CustomLists

fun EntryProviderScope<NavKey2>.customListEntry(navigator: Navigator) {
    entry<CustomListNavKey>(metadata = slideInHorizontalTransition()) {
        CustomLists(navigator = navigator)
    }

    createCustomListEntry(navigator)
    deleteCustomListEntry(navigator)
    editCustomListEntry(navigator)
    editCustomListLocationsEntry(navigator)
    editCustomListNameEntry(navigator)
    discardCustomListChangesEntry(navigator)
}
