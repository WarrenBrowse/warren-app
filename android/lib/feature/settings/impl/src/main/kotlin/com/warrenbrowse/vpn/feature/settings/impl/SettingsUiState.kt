package com.warrenbrowse.vpn.feature.settings.impl

data class SettingsUiState(
    val appVersion: String,
    val isLoggedIn: Boolean,
    val isSupportedVersion: Boolean,
    val isDaitaEnabled: Boolean,
    val isMultiHopEnabled: Boolean,
    val isPortForwardingEnabled: Boolean,
    val isPlayBuild: Boolean,
    /**
     * Newest version the signed update manifest offers, or null when the app is
     * current. Carried at the root so the App info row can say an update is
     * waiting without the user having to open the page.
     */
    val availableUpgrade: String? = null,
)
