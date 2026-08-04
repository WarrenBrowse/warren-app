package com.warrenbrowse.vpn.screen.navigation

import androidx.navigation3.runtime.EntryProviderScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.core.animation.slideUpModalTransition
import com.warrenbrowse.vpn.screen.outoftime.OutOfTimeScreen

fun EntryProviderScope<NavKey2>.outOfTimeEntry(navigator: Navigator) {
    // The gate rises over whatever the user was looking at and drops back off it
    // when credit lands, so it moves on the Y axis like the other overlays.
    entry<OutOfTimeNavKey>(metadata = slideUpModalTransition()) {
        OutOfTimeScreen(navigator = navigator)
    }
}
