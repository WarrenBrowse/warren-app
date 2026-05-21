package com.warrenbrowse.vpn.feature.addtime.impl.navigation

import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.navigation3.runtime.EntryProviderScope
import androidx.navigation3.scene.DialogSceneStrategy
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.addtime.api.VerificationPendingNavKey
import com.warrenbrowse.vpn.feature.addtime.impl.verificationpending.VerificationPending

@OptIn(ExperimentalMaterial3Api::class)
fun EntryProviderScope<NavKey2>.addTimeVerificationPendingEntry(navigator: Navigator) {
    entry<VerificationPendingNavKey>(metadata = DialogSceneStrategy.dialog()) {
        VerificationPending(navigator = navigator)
    }

    addTimeEntry(navigator)
}
