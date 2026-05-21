package com.warrenbrowse.vpn.feature.serveripoverride.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.serveripoverride.api.ImportOverrideByTextNavKey
import com.warrenbrowse.vpn.feature.serveripoverride.impl.importbytext.ImportOverridesByText

internal fun EntryProviderScope<NavKey2>.importOverrideByTextScreenEntry(navigator: Navigator) {
    entry<ImportOverrideByTextNavKey> { ImportOverridesByText(navigator = navigator) }
}
