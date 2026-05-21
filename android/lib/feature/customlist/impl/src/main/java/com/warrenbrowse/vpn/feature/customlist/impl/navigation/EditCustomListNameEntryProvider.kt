package com.warrenbrowse.vpn.feature.customlist.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import androidx.navigation3.scene.DialogSceneStrategy
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.customlist.api.EditCustomListNameNavKey
import com.warrenbrowse.vpn.feature.customlist.impl.screen.editname.EditCustomListName

internal fun EntryProviderScope<NavKey2>.editCustomListNameEntry(navigator: Navigator) {
    entry<EditCustomListNameNavKey>(metadata = DialogSceneStrategy.dialog()) { navKey ->
        EditCustomListName(navArgs = navKey, navigator = navigator)
    }
}
