package com.warrenbrowse.vpn.feature.settings.api

import com.warrenbrowse.vpn.core.NavKey2
import kotlinx.parcelize.Parcelize

/**
 * Navigation key for the D.5 wallet management screen reachable from
 * Settings. Hosts `WarrenWalletSettingsScreen` which embeds
 * `WarrenWalletSettingsSection` (View recovery phrase + Erase wallet).
 */
@Parcelize data object WarrenWalletSettingsNavKey : NavKey2
