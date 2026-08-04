package com.warrenbrowse.vpn.feature.settings.api

import com.warrenbrowse.vpn.core.NavKey2
import kotlinx.parcelize.Parcelize

/**
 * Navigation key for the Warren account screen, reachable from the Connect
 * header account icon and from the Settings "Wallet" entry. Hosts
 * `WarrenWalletSettings`: identity (public key + paid-until) plus the account
 * actions (buy credit, redeem voucher, view recovery phrase, erase wallet).
 */
@Parcelize data object WarrenWalletSettingsNavKey : NavKey2
