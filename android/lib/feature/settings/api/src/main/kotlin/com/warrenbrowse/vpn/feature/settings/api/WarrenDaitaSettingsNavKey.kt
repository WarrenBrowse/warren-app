package com.warrenbrowse.vpn.feature.settings.api

import com.warrenbrowse.vpn.core.NavKey2
import kotlinx.parcelize.Parcelize

/**
 * Navigation key for the dedicated DAITA settings page (desktop
 * `DaitaSettingsView` parity): explainer plus the single Enable switch
 * read by `WarrenTunnelConfigBuilder` at connect time.
 */
@Parcelize data object WarrenDaitaSettingsNavKey : NavKey2
