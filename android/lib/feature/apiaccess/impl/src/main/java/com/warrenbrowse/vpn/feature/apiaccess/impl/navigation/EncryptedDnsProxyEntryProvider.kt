package com.warrenbrowse.vpn.feature.apiaccess.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import androidx.navigation3.scene.DialogSceneStrategy
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.apiaccess.api.EncryptedDnsProxyInfoNavKey
import com.warrenbrowse.vpn.feature.apiaccess.impl.screen.edpinfo.EncryptedDnsProxyInfo

internal fun EntryProviderScope<NavKey2>.encryptedDnsProxyAccessEntry(navigator: Navigator) {
    entry<EncryptedDnsProxyInfoNavKey>(metadata = DialogSceneStrategy.dialog()) {
        EncryptedDnsProxyInfo(navigator = navigator)
    }
}
