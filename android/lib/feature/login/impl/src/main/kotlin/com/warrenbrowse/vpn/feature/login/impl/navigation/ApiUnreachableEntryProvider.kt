package com.warrenbrowse.vpn.feature.login.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.login.api.ApiUnreachableNavKey
import com.warrenbrowse.vpn.feature.login.impl.apiunreachable.ApiUnreachableInfo

internal fun EntryProviderScope<NavKey2>.apiUnreachableEntry(navigator: Navigator) {
    entry<ApiUnreachableNavKey> { navKey ->
        ApiUnreachableInfo(navigator = navigator, navArgs = navKey)
    }
}
