package com.warrenbrowse.vpn.feature.daita.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import androidx.navigation3.scene.DialogSceneStrategy
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.daita.api.DaitaDirectOnlyConfirmationNavKey
import com.warrenbrowse.vpn.feature.daita.impl.DaitaDirectOnlyConfirmation

fun EntryProviderScope<NavKey2>.daitaDirectOnlyConfirmationEntry(navigator: Navigator) {
    entry<DaitaDirectOnlyConfirmationNavKey>(metadata = DialogSceneStrategy.dialog()) {
        DaitaDirectOnlyConfirmation(navigator = navigator)
    }
}
