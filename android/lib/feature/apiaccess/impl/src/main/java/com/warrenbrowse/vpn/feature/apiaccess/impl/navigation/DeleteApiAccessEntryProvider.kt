package com.warrenbrowse.vpn.feature.apiaccess.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import androidx.navigation3.scene.DialogSceneStrategy
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.apiaccess.api.DeleteApiAccessMethodNavKey
import com.warrenbrowse.vpn.feature.apiaccess.impl.screen.delete.DeleteApiAccessMethodConfirmation

internal fun EntryProviderScope<NavKey2>.deleteApiAccessEntry(navigator: Navigator) {
    entry<DeleteApiAccessMethodNavKey>(metadata = DialogSceneStrategy.dialog()) { navKey ->
        DeleteApiAccessMethodConfirmation(
            apiAccessMethodId = navKey.apiAccessMethodId,
            navigator = navigator,
        )
    }
}
