package com.warrenbrowse.vpn.feature.apiaccess.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.core.animation.slideInHorizontalTransition
import com.warrenbrowse.vpn.core.scene.ListDetailSceneStrategy
import com.warrenbrowse.vpn.feature.apiaccess.api.ApiAccessNavKey
import com.warrenbrowse.vpn.feature.apiaccess.impl.screen.list.ApiAccessList

fun EntryProviderScope<NavKey2>.apiAccessEntry(navigator: Navigator) {
    entry<ApiAccessNavKey>(
        metadata = ListDetailSceneStrategy.detailPane() + slideInHorizontalTransition()
    ) {
        ApiAccessList(navigator = navigator)
    }

    apiAccessMethodDetailsEntry(navigator)
    apiAccessMethodInfoEntry(navigator)
    editApiAccessMethodEntry(navigator)
    deleteApiAccessEntry(navigator)
    discardApiAccessChangesEntry(navigator)
    encryptedDnsProxyAccessEntry(navigator)
    saveApiAccessMethodEntry(navigator)
}
