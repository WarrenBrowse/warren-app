package com.warrenbrowse.vpn.feature.customlist.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import androidx.navigation3.scene.DialogSceneStrategy
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.customlist.api.DiscardCustomListChangesNavKey
import com.warrenbrowse.vpn.feature.customlist.impl.screen.discard.DiscardChanges

internal fun EntryProviderScope<NavKey2>.discardCustomListChangesEntry(navigator: Navigator) {
    entry<DiscardCustomListChangesNavKey>(metadata = DialogSceneStrategy.dialog()) {
        DiscardChanges(navigator = navigator)
    }
}
