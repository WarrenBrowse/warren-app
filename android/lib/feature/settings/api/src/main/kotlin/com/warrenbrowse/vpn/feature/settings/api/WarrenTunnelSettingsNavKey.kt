package com.warrenbrowse.vpn.feature.settings.api

import com.warrenbrowse.vpn.core.NavKey2
import kotlinx.parcelize.Parcelize

/**
 * Navigation key for the D.4 step 8 Warren tunnel settings screen.
 * Hosts the DAITA / NAT-PMP / multi-hop / M4.0 toggles read by
 * `WarrenTunnelConfigBuilder` at connect time.
 */
@Parcelize data object WarrenTunnelSettingsNavKey : NavKey2
