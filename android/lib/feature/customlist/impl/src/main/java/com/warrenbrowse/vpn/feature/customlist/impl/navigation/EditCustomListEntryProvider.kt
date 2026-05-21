package com.warrenbrowse.vpn.feature.customlist.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.core.animation.slideInHorizontalTransition
import com.warrenbrowse.vpn.feature.customlist.api.EditCustomListNavKey
import com.warrenbrowse.vpn.feature.customlist.impl.screen.editlist.EditCustomList

internal fun EntryProviderScope<NavKey2>.editCustomListEntry(navigator: Navigator) {
    entry<EditCustomListNavKey>(metadata = slideInHorizontalTransition()) { navKey ->
        EditCustomList(customListId = navKey.customListId, navigator = navigator)
    }
}
