package com.warrenbrowse.vpn.feature.deleteaccount.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.core.animation.slideInHorizontalTransition
import com.warrenbrowse.vpn.feature.deleteaccount.api.DeleteAccountNavKey
import com.warrenbrowse.vpn.feature.deleteaccount.impl.DeleteAccount

fun EntryProviderScope<NavKey2>.deleteAccountEntry(navigator: Navigator) {
    entry<DeleteAccountNavKey>(metadata = slideInHorizontalTransition()) {
        DeleteAccount(navigator = navigator)
    }

    deleteAccountCompleteEntry(navigator)
    deleteAccountConfirmationEntry(navigator)
}
