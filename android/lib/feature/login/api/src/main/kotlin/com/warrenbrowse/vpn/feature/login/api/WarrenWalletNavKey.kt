package com.warrenbrowse.vpn.feature.login.api

import com.warrenbrowse.vpn.core.NavKey2
import kotlinx.parcelize.Parcelize

/**
 * Navigation key for the wallet entry screen (Generate / Restore branches).
 * Hosts `WarrenWalletLoginScreen` from `lib/feature/login/impl`.
 *
 * Replaces the legacy [LoginNavKey] flow for first-launch onboarding when
 * no wallet has been persisted (Warren has no account-number model;
 * identity = BIP39 mnemonic). Existing [LoginNavKey] entries remain
 * routed for backward-compat while D.5 is co-existing with the Mullvad
 * account-number scaffold being deleted.
 */
@Parcelize data object WarrenWalletNavKey : NavKey2
