package com.warrenbrowse.vpn.feature.daita.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import androidx.navigation3.scene.DialogSceneStrategy
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.daita.api.DaitaDirectOnlyInfoNavKey
import com.warrenbrowse.vpn.feature.daita.impl.DaitaDirectOnlyInfo

fun EntryProviderScope<NavKey2>.daitaDirectOnlyInfoEntry(navigator: Navigator) {
    entry<DaitaDirectOnlyInfoNavKey>(metadata = DialogSceneStrategy.dialog()) {
        DaitaDirectOnlyInfo(navigator = navigator)
    }
}
