package com.warrenbrowse.vpn.feature.login.api

import com.warrenbrowse.vpn.core.NavKey2
import kotlinx.parcelize.Parcelize

/**
 * Navigation key for the recovery-phrase screen reached from the account page
 * (desktop `RoutePath.keys`). A destination rather than a dialog so back
 * semantics come from the stack, and so it can push the restore screen.
 *
 * The key is a sentinel and deliberately carries no mnemonic: the caller
 * unlocks the wallet, stages the phrase in
 * [com.warrenbrowse.vpn.lib.repository.MnemonicCache] and navigates here, for
 * the same reason [WarrenWalletBackupNavKey] does.
 */
@Parcelize object WarrenKeysNavKey : NavKey2

/**
 * Navigation key for restoring another wallet from its recovery phrase without
 * erasing the current one first (desktop `RoutePath.restoreKeys`).
 */
@Parcelize object WarrenRestoreMnemonicNavKey : NavKey2
