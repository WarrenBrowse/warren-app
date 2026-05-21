package com.warrenbrowse.vpn.feature.serveripoverride.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import androidx.navigation3.ui.LocalNavAnimatedContentScope
import com.warrenbrowse.vpn.common.compose.LocalSharedTransitionScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.core.animation.slideInHorizontalTransition
import com.warrenbrowse.vpn.core.scene.ListDetailSceneStrategy
import com.warrenbrowse.vpn.feature.serveripoverride.api.ServerIpOverrideNavKey
import com.warrenbrowse.vpn.feature.serveripoverride.impl.ServerIpOverrides

fun EntryProviderScope<NavKey2>.serverIpOverrideEntry(navigator: Navigator) {
    entry<ServerIpOverrideNavKey>(
        metadata = ListDetailSceneStrategy.detailPane() + slideInHorizontalTransition()
    ) { navKey ->
        LocalSharedTransitionScope.current?.ServerIpOverrides(
            navArgs = navKey,
            navigator = navigator,
            animatedVisibilityScope = LocalNavAnimatedContentScope.current,
        )
    }

    resetServerIpOverrideConfirmationEntry(navigator)
    importOverrideByTextScreenEntry(navigator)
    importOverridesEntry(navigator)
    serverIpOverrideInfoEntry(navigator)
}
