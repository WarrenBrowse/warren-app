package com.warrenbrowse.vpn.feature.home.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.home.api.WelcomeNavKey
import com.warrenbrowse.vpn.feature.home.impl.welcome.Welcome

internal fun EntryProviderScope<NavKey2>.welcomeEntry(navigator: Navigator) {
    entry<WelcomeNavKey> { Welcome(navigator = navigator) }
}
