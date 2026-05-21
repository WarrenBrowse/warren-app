package com.warrenbrowse.vpn.feature.login.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import androidx.navigation3.scene.DialogSceneStrategy
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.login.api.CreateAccountConfirmationNavKey
import com.warrenbrowse.vpn.feature.login.impl.CreateAccountConfirmation

internal fun EntryProviderScope<NavKey2>.createAccountConfirmationEntry(navigator: Navigator) {
    entry<CreateAccountConfirmationNavKey>(metadata = DialogSceneStrategy.dialog()) {
        CreateAccountConfirmation(navigator = navigator)
    }
}
