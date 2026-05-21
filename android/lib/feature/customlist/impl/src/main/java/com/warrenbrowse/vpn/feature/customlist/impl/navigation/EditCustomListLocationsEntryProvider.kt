package com.warrenbrowse.vpn.feature.customlist.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.customlist.api.EditCustomListLocationsNavKey
import com.warrenbrowse.vpn.feature.customlist.impl.screen.editlocations.CustomListLocations

internal fun EntryProviderScope<NavKey2>.editCustomListLocationsEntry(navigator: Navigator) {
    entry<EditCustomListLocationsNavKey> { navKey ->
        CustomListLocations(navArgs = navKey, navigator = navigator)
    }
}
