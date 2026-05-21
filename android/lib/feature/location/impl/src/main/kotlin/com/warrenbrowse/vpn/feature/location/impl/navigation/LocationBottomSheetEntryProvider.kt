package com.warrenbrowse.vpn.feature.location.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.core.scene.SingleOverlaySceneStrategy
import com.warrenbrowse.vpn.feature.location.api.LocationBottomSheetNavKey
import com.warrenbrowse.vpn.feature.location.impl.bottomsheet.LocationBottomSheets

internal fun EntryProviderScope<NavKey2>.locationBottomSheetEntry(navigator: Navigator) {
    entry<LocationBottomSheetNavKey>(metadata = SingleOverlaySceneStrategy.overlay()) { navKey ->
        LocationBottomSheets(navigator = navigator, locationBottomSheetState = navKey.state)
    }
}
