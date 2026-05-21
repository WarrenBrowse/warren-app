package com.warrenbrowse.vpn.feature.apiaccess.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import androidx.navigation3.scene.DialogSceneStrategy
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.apiaccess.api.SaveApiAccessMethodNavKey
import com.warrenbrowse.vpn.feature.apiaccess.impl.screen.save.SaveApiAccessMethod

internal fun EntryProviderScope<NavKey2>.saveApiAccessMethodEntry(navigator: Navigator) {
    entry<SaveApiAccessMethodNavKey>(metadata = DialogSceneStrategy.dialog()) { navKey ->
        SaveApiAccessMethod(navArgs = navKey, navigator = navigator)
    }
}
