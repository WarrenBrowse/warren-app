package com.warrenbrowse.vpn.feature.settings.api

import com.warrenbrowse.vpn.core.NavKey2
import kotlinx.parcelize.Parcelize

/**
 * Navigation key for the dedicated Multihop settings page (desktop
 * `WarrenMultiHopSettingsView` parity). Multi-hop is always on for Warren,
 * so the page carries the always-on explainer plus the entry/exit country
 * pickers, not a toggle.
 */
@Parcelize data object WarrenMultihopSettingsNavKey : NavKey2
