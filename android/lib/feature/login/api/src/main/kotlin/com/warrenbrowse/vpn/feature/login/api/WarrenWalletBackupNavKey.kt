package com.warrenbrowse.vpn.feature.login.api

import com.warrenbrowse.vpn.core.NavKey2
import kotlinx.parcelize.Parcelize

/**
 * Navigation key for the freshly-generated-mnemonic backup screen
 * (`WarrenWalletBackupScreen` in `lib/feature/login/impl`).
 *
 * The NavKey is a sentinel and deliberately carries no mnemonic. Compose
 * Navigation persists NavKeys in the saved-state Bundle on process-kill, so
 * an inline phrase would live in a system-managed bundle, outside the
 * [com.warrenbrowse.vpn.lib.model.wallet.Mnemonic] zero-on-close lifecycle.
 * The mnemonic is instead handed off out-of-band through
 * [com.warrenbrowse.vpn.lib.repository.MnemonicCache] so it never touches the
 * Bundle. A process-kill empties the cache and the backup screen gracefully
 * falls back to the login screen (the user has to re-create or re-import
 * after a process death anyway).
 */
@Parcelize
data class WarrenWalletBackupNavKey(
    // Carried from [WarrenWalletNavKey] so the post-backup step routes into the
    // onboarding wizard on first run, or straight to Connect otherwise.
    val onboarding: Boolean = false,
) : NavKey2
