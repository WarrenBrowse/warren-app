package com.warrenbrowse.vpn.feature.apiaccess.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.core.animation.slideInHorizontalTransition
import com.warrenbrowse.vpn.feature.apiaccess.api.ApiAccessMethodDetailsNavKey
import com.warrenbrowse.vpn.feature.apiaccess.impl.screen.detail.ApiAccessMethodDetails

internal fun EntryProviderScope<NavKey2>.apiAccessMethodDetailsEntry(navigator: Navigator) {
    entry<ApiAccessMethodDetailsNavKey>(metadata = slideInHorizontalTransition()) { navKey ->
        ApiAccessMethodDetails(apiAccessMethodId = navKey.accessMethodId, navigator = navigator)
    }
}
