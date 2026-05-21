package com.warrenbrowse.vpn.feature.daita.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import androidx.navigation3.ui.LocalNavAnimatedContentScope
import com.warrenbrowse.vpn.common.compose.LocalSharedTransitionScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.core.animation.slideInHorizontalTransition
import com.warrenbrowse.vpn.core.scene.ListDetailSceneStrategy
import com.warrenbrowse.vpn.feature.daita.api.DaitaNavKey
import com.warrenbrowse.vpn.feature.daita.impl.Daita

fun EntryProviderScope<NavKey2>.daitaEntry(navigator: Navigator) {
    entry<DaitaNavKey>(
        metadata = ListDetailSceneStrategy.detailPane() + slideInHorizontalTransition()
    ) { navKey ->
        LocalSharedTransitionScope.current?.Daita(
            navigator = navigator,
            isModal = navKey.isModal,
            animatedVisibilityScope = LocalNavAnimatedContentScope.current,
        )
    }

    daitaDirectOnlyConfirmationEntry(navigator)
    daitaDirectOnlyInfoEntry(navigator)
}
