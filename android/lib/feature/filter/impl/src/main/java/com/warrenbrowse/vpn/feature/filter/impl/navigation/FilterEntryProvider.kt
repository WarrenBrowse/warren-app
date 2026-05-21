package com.warrenbrowse.vpn.feature.filter.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.core.animation.slideInHorizontalTransition
import com.warrenbrowse.vpn.feature.filter.api.FilterNavKey
import com.warrenbrowse.vpn.feature.filter.impl.Filter

fun EntryProviderScope<NavKey2>.filterEntry(navigator: Navigator) {
    entry<FilterNavKey>(metadata = slideInHorizontalTransition()) { Filter(navigator = navigator) }
}
