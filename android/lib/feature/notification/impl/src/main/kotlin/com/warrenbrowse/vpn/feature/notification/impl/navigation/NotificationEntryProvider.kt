package com.warrenbrowse.vpn.feature.notification.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.core.animation.slideInHorizontalTransition
import com.warrenbrowse.vpn.core.scene.ListDetailSceneStrategy
import com.warrenbrowse.vpn.feature.notification.api.NotificationSettingsNavKey
import com.warrenbrowse.vpn.feature.notification.impl.NotificationSettings

fun EntryProviderScope<NavKey2>.notificationEntry(navigator: Navigator) {
    entry<NotificationSettingsNavKey>(
        metadata = ListDetailSceneStrategy.detailPane() + slideInHorizontalTransition()
    ) {
        NotificationSettings(navigator = navigator)
    }
}
