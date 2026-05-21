package com.warrenbrowse.vpn.feature.splittunneling.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import androidx.navigation3.ui.LocalNavAnimatedContentScope
import com.warrenbrowse.vpn.common.compose.LocalSharedTransitionScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.core.animation.slideInHorizontalTransition
import com.warrenbrowse.vpn.core.scene.ListDetailSceneStrategy
import com.warrenbrowse.vpn.feature.splittunneling.api.SplitTunnelingNavKey
import com.warrenbrowse.vpn.feature.splittunneling.impl.SplitTunneling

fun EntryProviderScope<NavKey2>.splitTunnelingEntry(navigator: Navigator) {
    entry<SplitTunnelingNavKey>(
        metadata = ListDetailSceneStrategy.detailPane() + slideInHorizontalTransition()
    ) { navArgs ->
        LocalSharedTransitionScope.current?.SplitTunneling(
            isModal = navArgs.isModal,
            navigator = navigator,
            animatedVisibilityScope = LocalNavAnimatedContentScope.current,
        )
    }
    searchSplitTunnelingEntry(navigator)
}
