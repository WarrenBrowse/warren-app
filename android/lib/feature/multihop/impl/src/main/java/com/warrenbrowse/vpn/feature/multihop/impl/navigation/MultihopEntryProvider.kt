package com.warrenbrowse.vpn.feature.multihop.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import androidx.navigation3.ui.LocalNavAnimatedContentScope
import com.warrenbrowse.vpn.common.compose.LocalSharedTransitionScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.core.animation.slideInHorizontalTransition
import com.warrenbrowse.vpn.core.scene.ListDetailSceneStrategy
import com.warrenbrowse.vpn.feature.multihop.api.MultihopNavKey
import com.warrenbrowse.vpn.feature.multihop.impl.Multihop

fun EntryProviderScope<NavKey2>.multihopEntry(navigator: Navigator) {
    entry<MultihopNavKey>(
        metadata = ListDetailSceneStrategy.detailPane() + slideInHorizontalTransition()
    ) { navKey ->
        LocalSharedTransitionScope.current?.Multihop(
            isModal = navKey.isModal,
            navigator = navigator,
            animatedVisibilityScope = LocalNavAnimatedContentScope.current,
        )
    }
}
