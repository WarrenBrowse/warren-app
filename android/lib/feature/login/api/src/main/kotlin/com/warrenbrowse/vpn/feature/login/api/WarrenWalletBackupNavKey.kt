package com.warrenbrowse.vpn.feature.login.api

import com.warrenbrowse.vpn.core.NavKey2
import kotlinx.parcelize.Parcelize

/**
 * Navigation key for the freshly-generated-mnemonic backup screen
 * (`WarrenWalletBackupScreen` in `lib/feature/login/impl`).
 *
 * Audit follow-up: the NavKey USED to carry the mnemonic phrase
 * inline (`data class ... (val mnemonicPhrase: String)`). Compose
 * Navigation persists NavKeys in the saved-state Bundle on
 * process-kill — so the cleartext phrase would live in a system-
 * managed bundle, completely outside the [com.warrenbrowse.vpn.lib.model.wallet.Mnemonic]
 * zero-on-close lifecycle. The NavKey is now a sentinel: the
 * mnemonic is handed off out-of-band through
 * [com.warrenbrowse.vpn.lib.repository.MnemonicCache] so it never
 * touches the Bundle. A process-kill empties the cache and the
 * backup screen gracefully falls back to the login screen (which is
 * the right behaviour — the user has to re-create or re-import
 * after a process death anyway).
 */
@Parcelize
data object WarrenWalletBackupNavKey : NavKey2
