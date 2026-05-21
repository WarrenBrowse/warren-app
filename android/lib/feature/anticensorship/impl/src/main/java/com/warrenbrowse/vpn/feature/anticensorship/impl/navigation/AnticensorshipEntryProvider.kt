package com.warrenbrowse.vpn.feature.anticensorship.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import androidx.navigation3.ui.LocalNavAnimatedContentScope
import com.warrenbrowse.vpn.common.compose.LocalSharedTransitionScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.core.animation.slideInHorizontalTransition
import com.warrenbrowse.vpn.core.scene.ListDetailSceneStrategy
import com.warrenbrowse.vpn.feature.anticensorship.api.AntiCensorshipNavKey
import com.warrenbrowse.vpn.feature.anticensorship.impl.AntiCensorshipSettings

fun EntryProviderScope<NavKey2>.anticensorshipEntry(navigator: Navigator) {
    entry<AntiCensorshipNavKey>(
        metadata = ListDetailSceneStrategy.detailPane() + slideInHorizontalTransition()
    ) { navKey ->
        LocalSharedTransitionScope.current?.AntiCensorshipSettings(
            navigator = navigator,
            navArgs = navKey,
            animatedVisibilityScope = LocalNavAnimatedContentScope.current,
        )
    }

    customPortEntry(navigator)
    selectPortEntry(navigator)
}
