package com.warrenbrowse.vpn.feature.account.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.core.animation.accountTransition
import com.warrenbrowse.vpn.feature.account.api.AccountNavKey
import com.warrenbrowse.vpn.feature.account.impl.Account

fun EntryProviderScope<NavKey2>.accountEntry(navigator: Navigator) {
    entry<AccountNavKey>(metadata = accountTransition()) { Account(navigator = navigator) }
}
