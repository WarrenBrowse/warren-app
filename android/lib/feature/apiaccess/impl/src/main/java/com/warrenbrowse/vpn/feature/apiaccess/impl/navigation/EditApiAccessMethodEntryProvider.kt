package com.warrenbrowse.vpn.feature.apiaccess.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.core.animation.slideInHorizontalTransition
import com.warrenbrowse.vpn.feature.apiaccess.api.EditApiAccessMethodNavKey
import com.warrenbrowse.vpn.feature.apiaccess.impl.screen.edit.EditApiAccessMethod

internal fun EntryProviderScope<NavKey2>.editApiAccessMethodEntry(navigator: Navigator) {
    entry<EditApiAccessMethodNavKey>(metadata = slideInHorizontalTransition()) { navKey ->
        EditApiAccessMethod(apiAccessMethodId = navKey.accessMethodId, navigator = navigator)
    }
}
