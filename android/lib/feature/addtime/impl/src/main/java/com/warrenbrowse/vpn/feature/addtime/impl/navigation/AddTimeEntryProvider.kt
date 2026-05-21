package com.warrenbrowse.vpn.feature.addtime.impl.navigation

import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.navigation3.runtime.EntryProviderScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.core.scene.SingleOverlaySceneStrategy
import com.warrenbrowse.vpn.feature.addtime.api.AddTimeNavKey
import com.warrenbrowse.vpn.feature.addtime.impl.AddTimeBottomSheet

@OptIn(ExperimentalMaterial3Api::class)
fun EntryProviderScope<NavKey2>.addTimeEntry(navigator: Navigator) {
    entry<AddTimeNavKey>(metadata = SingleOverlaySceneStrategy.overlay()) {
        AddTimeBottomSheet(navigator = navigator)
    }
}
