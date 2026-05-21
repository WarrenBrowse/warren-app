package com.warrenbrowse.vpn.feature.deleteaccount.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.core.animation.slideInHorizontalTransition
import com.warrenbrowse.vpn.feature.deleteaccount.api.DeleteAccountCompleteNavKey
import com.warrenbrowse.vpn.feature.deleteaccount.impl.deleteaccountcomplete.DeleteAccountComplete

internal fun EntryProviderScope<NavKey2>.deleteAccountCompleteEntry(navigator: Navigator) {
    entry<DeleteAccountCompleteNavKey>(metadata = slideInHorizontalTransition()) {
        DeleteAccountComplete(navigator = navigator)
    }
}
