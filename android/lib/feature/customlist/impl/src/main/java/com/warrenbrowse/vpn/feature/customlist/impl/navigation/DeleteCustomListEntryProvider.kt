package com.warrenbrowse.vpn.feature.customlist.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import androidx.navigation3.scene.DialogSceneStrategy
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.customlist.api.DeleteCustomListNavKey
import com.warrenbrowse.vpn.feature.customlist.impl.screen.delete.DeleteCustomList

internal fun EntryProviderScope<NavKey2>.deleteCustomListEntry(navigator: Navigator) {
    entry<DeleteCustomListNavKey>(metadata = DialogSceneStrategy.dialog()) { navKey ->
        DeleteCustomList(navArgs = navKey, navigator = navigator)
    }
}
