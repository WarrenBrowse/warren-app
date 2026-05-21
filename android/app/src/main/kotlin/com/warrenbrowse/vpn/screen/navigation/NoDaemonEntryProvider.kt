package com.warrenbrowse.vpn.screen.navigation

import androidx.navigation3.runtime.EntryProviderScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.screen.nodaemon.NoDaemon

fun EntryProviderScope<NavKey2>.noDaemonEntry(navigator: Navigator) {
    entry<NoDaemonNavKey> { NoDaemon(navigator = navigator) }
}
