package com.warrenbrowse.vpn.feature.settings.api

import com.warrenbrowse.vpn.core.NavKey2
import kotlinx.parcelize.Parcelize

/**
 * Navigation key for the dedicated Port forwarding settings page (desktop
 * `PortForwardingSettingsView` parity): the enable switch plus the Android
 * preferred-port / protocol / lifetime controls read by the NAT-PMP refresh
 * loop at connect time.
 */
@Parcelize data object WarrenPortForwardingSettingsNavKey : NavKey2
