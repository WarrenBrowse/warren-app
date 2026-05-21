package com.warrenbrowse.vpn.feature.redeemvoucher.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import androidx.navigation3.scene.DialogSceneStrategy
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.redeemvoucher.api.RedeemVoucherNavKey
import com.warrenbrowse.vpn.feature.redeemvoucher.impl.RedeemVoucher

fun EntryProviderScope<NavKey2>.redeemVoucherEntry(navigator: Navigator) {
    entry<RedeemVoucherNavKey>(metadata = DialogSceneStrategy.dialog()) {
        RedeemVoucher(navigator = navigator)
    }
}
