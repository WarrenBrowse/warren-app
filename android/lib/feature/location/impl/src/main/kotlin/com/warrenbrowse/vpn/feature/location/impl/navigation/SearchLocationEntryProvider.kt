package com.warrenbrowse.vpn.feature.location.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.location.api.SearchLocationNavKey
import com.warrenbrowse.vpn.feature.location.impl.search.SearchLocation

internal fun EntryProviderScope<NavKey2>.searchLocationEntry(navigator: Navigator) {
    entry<SearchLocationNavKey> { navKey ->
        SearchLocation(relayListType = navKey.relayListType, navigator = navigator)
    }
}
