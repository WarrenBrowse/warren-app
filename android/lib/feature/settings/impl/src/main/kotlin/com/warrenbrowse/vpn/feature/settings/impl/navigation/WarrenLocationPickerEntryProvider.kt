package com.warrenbrowse.vpn.feature.settings.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.core.animation.slideUpModalTransition
import com.warrenbrowse.vpn.feature.settings.api.WarrenLocationPickerNavKey
import com.warrenbrowse.vpn.feature.settings.impl.WarrenLocationPicker

fun EntryProviderScope<NavKey2>.warrenLocationPickerEntry(navigator: Navigator) {
    // The picker is a temporary overlay over whatever asked for a location, not a level
    // deeper in the settings tree, so it rises and drops on the Y axis.
    entry<WarrenLocationPickerNavKey>(metadata = slideUpModalTransition()) { navKey ->
        WarrenLocationPicker(navigator = navigator, connectOnPick = navKey.connectOnPick)
    }
}
