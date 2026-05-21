package com.warrenbrowse.vpn.feature.home.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.home.api.OutOfTimeNavKey
import com.warrenbrowse.vpn.feature.home.impl.outoftime.OutOfTime

internal fun EntryProviderScope<NavKey2>.outOfTimeEntry(navigator: Navigator) {
    entry<OutOfTimeNavKey> { OutOfTime(navigator = navigator) }
}
