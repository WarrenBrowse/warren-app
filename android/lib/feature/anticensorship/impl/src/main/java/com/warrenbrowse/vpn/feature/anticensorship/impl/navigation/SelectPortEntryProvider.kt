package com.warrenbrowse.vpn.feature.anticensorship.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.core.animation.slideInHorizontalTransition
import com.warrenbrowse.vpn.feature.anticensorship.api.SelectPortNavKey
import com.warrenbrowse.vpn.feature.anticensorship.impl.selectport.SelectPort

internal fun EntryProviderScope<NavKey2>.selectPortEntry(navigator: Navigator) {
    entry<SelectPortNavKey>(metadata = slideInHorizontalTransition()) { navArgs ->
        SelectPort(navArgs = navArgs, navigator = navigator)
    }
}
