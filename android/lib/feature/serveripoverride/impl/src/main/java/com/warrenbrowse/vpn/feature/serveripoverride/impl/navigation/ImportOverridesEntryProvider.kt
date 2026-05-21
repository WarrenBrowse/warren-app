package com.warrenbrowse.vpn.feature.serveripoverride.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.core.scene.SingleOverlaySceneStrategy
import com.warrenbrowse.vpn.feature.serveripoverride.api.ImportOverridesNavKey
import com.warrenbrowse.vpn.feature.serveripoverride.impl.ImportOverridesBottomSheet

internal fun EntryProviderScope<NavKey2>.importOverridesEntry(navigator: Navigator) {
    entry<ImportOverridesNavKey>(metadata = SingleOverlaySceneStrategy.overlay()) {
        ImportOverridesBottomSheet(navigator = navigator, overridesActive = it.overridesActive)
    }
}
