package com.warrenbrowse.vpn.feature.deleteaccount.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.core.animation.slideInHorizontalTransition
import com.warrenbrowse.vpn.feature.deleteaccount.api.DeleteAccountConfirmationNavKey
import com.warrenbrowse.vpn.feature.deleteaccount.impl.deleteaccountconfirmation.DeleteAccountConfirmation

internal fun EntryProviderScope<NavKey2>.deleteAccountConfirmationEntry(navigator: Navigator) {
    entry<DeleteAccountConfirmationNavKey>(metadata = slideInHorizontalTransition()) {
        DeleteAccountConfirmation(navigator = navigator)
    }
}
