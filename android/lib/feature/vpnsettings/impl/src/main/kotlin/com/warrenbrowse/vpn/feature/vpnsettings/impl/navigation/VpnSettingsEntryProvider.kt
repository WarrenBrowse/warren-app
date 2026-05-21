package com.warrenbrowse.vpn.feature.vpnsettings.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import androidx.navigation3.ui.LocalNavAnimatedContentScope
import com.warrenbrowse.vpn.common.compose.LocalSharedTransitionScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.core.animation.slideInHorizontalTransition
import com.warrenbrowse.vpn.core.scene.ListDetailSceneStrategy
import com.warrenbrowse.vpn.feature.vpnsettings.api.VpnSettingsNavKey
import com.warrenbrowse.vpn.feature.vpnsettings.impl.VpnSettings

fun EntryProviderScope<NavKey2>.vpnSettingsEntry(navigator: Navigator) {
    entry<VpnSettingsNavKey>(
        metadata = ListDetailSceneStrategy.listPane() + slideInHorizontalTransition()
    ) { navArgs ->
        LocalSharedTransitionScope.current?.VpnSettings(
            navArgs = navArgs,
            navigator = navigator,
            animatedVisibilityScope = LocalNavAnimatedContentScope.current,
        )
    }

    connectOnStartupInfoEntry(navigator)
    contentBlockersInfoEntry(navigator)
    customDnsInfoEntry(navigator)
    deviceIpInfoEntry(navigator)
    dnsEntry(navigator)
    localNetworkSharingInfoEntry(navigator)
    ipv6InfoEntry(navigator)
    malwareInfoEntry(navigator)
    mtuEntry(navigator)
    quantumResistanceInfoEntry(navigator)
}
