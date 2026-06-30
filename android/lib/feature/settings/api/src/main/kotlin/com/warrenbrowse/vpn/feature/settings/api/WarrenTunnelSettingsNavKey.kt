package com.warrenbrowse.vpn.feature.settings.api

import com.warrenbrowse.vpn.core.NavKey2
import kotlinx.parcelize.Parcelize

/**
 * Navigation key for the Warren tunnel settings screen. Hosts the
 * DAITA / NAT-PMP / multi-hop / obfuscation toggles read by
 * `WarrenTunnelConfigBuilder` at connect time.
 */
@Parcelize data object WarrenTunnelSettingsNavKey : NavKey2
