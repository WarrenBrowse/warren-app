package com.warrenbrowse.vpn.feature.location.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.core.animation.topLevelTransition
import com.warrenbrowse.vpn.feature.location.api.SelectLocationNavKey
import com.warrenbrowse.vpn.feature.location.impl.SelectLocation

fun EntryProviderScope<NavKey2>.selectLocationEntry(navigator: Navigator) {
    entry<SelectLocationNavKey>(metadata = topLevelTransition()) {
        SelectLocation(navigator = navigator)
    }

    locationBottomSheetEntry(navigator)
    searchLocationEntry(navigator)
}
