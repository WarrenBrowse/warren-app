package com.warrenbrowse.vpn.feature.settings.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.settings.api.WarrenLocationPickerNavKey
import com.warrenbrowse.vpn.feature.settings.impl.WarrenLocationPicker

fun EntryProviderScope<NavKey2>.warrenLocationPickerEntry(navigator: Navigator) {
    entry<WarrenLocationPickerNavKey> {
        WarrenLocationPicker(navigator = navigator)
    }
}
