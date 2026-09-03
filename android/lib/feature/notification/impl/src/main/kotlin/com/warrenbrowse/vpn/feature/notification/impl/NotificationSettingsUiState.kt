package com.warrenbrowse.vpn.feature.notification.impl

data class NotificationSettingsUiState(
    val locationInNotificationEnabled: Boolean,
    /**
     * The forum switch, or null for a wallet with no forum account: to
     * everyone else it would be a switch over a feature they have never
     * seen, naming a place they have not signed up to.
     */
    val forumNotificationsEnabled: Boolean? = null,
)
