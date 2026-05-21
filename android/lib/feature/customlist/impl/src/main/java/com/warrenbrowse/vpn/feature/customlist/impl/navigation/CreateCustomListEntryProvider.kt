package com.warrenbrowse.vpn.feature.customlist.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import androidx.navigation3.scene.DialogSceneStrategy
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.customlist.api.CreateCustomListNavKey
import com.warrenbrowse.vpn.feature.customlist.impl.screen.create.CreateCustomList

internal fun EntryProviderScope<NavKey2>.createCustomListEntry(navigator: Navigator) {
    entry<CreateCustomListNavKey>(metadata = DialogSceneStrategy.dialog()) { navKey ->
        CreateCustomList(locationCode = navKey.locationCode, navigator = navigator)
    }
}
