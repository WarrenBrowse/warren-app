package com.warrenbrowse.vpn.feature.settings.api

import com.warrenbrowse.vpn.core.NavKey2
import kotlinx.parcelize.Parcelize

/**
 * Navigation key for the VPN settings page (desktop `VpnSettingsView`
 * parity): local network sharing, DNS content blockers + custom DNS,
 * in-tunnel IPv6, kill switch / lockdown mode, anti-censorship, MTU and
 * the exit-key pin reset. DAITA, Multihop and Port forwarding each have
 * their own dedicated page. Kept under this name because the Connect
 * screen's feature indicators route here.
 */
@Parcelize data object WarrenTunnelSettingsNavKey : NavKey2
