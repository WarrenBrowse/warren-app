package com.warrenbrowse.vpn.feature.vpnsettings.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import androidx.navigation3.scene.DialogSceneStrategy
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.vpnsettings.api.DnsNavKey
import com.warrenbrowse.vpn.feature.vpnsettings.impl.dns.Dns

internal fun EntryProviderScope<NavKey2>.dnsEntry(navigator: Navigator) {
    entry<DnsNavKey>(metadata = DialogSceneStrategy.dialog()) { navKey ->
        Dns(navArgs = navKey, navigator = navigator)
    }
}
