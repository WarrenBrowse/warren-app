package com.warrenbrowse.vpn.feature.apiaccess.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import androidx.navigation3.scene.DialogSceneStrategy
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.apiaccess.api.DiscardApiAccessChangesNavKey
import com.warrenbrowse.vpn.feature.apiaccess.impl.screen.discardchanges.DiscardApiAccessChanges

internal fun EntryProviderScope<NavKey2>.discardApiAccessChangesEntry(navigator: Navigator) {
    entry<DiscardApiAccessChangesNavKey>(metadata = DialogSceneStrategy.dialog()) {
        DiscardApiAccessChanges(navigator = navigator)
    }
}
