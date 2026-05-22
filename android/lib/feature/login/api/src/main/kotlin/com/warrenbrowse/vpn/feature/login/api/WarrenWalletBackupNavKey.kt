package com.warrenbrowse.vpn.feature.login.api

import com.warrenbrowse.vpn.core.NavKey2
import kotlinx.parcelize.Parcelize

/**
 * Navigation key for the freshly-generated-mnemonic backup screen
 * (`WarrenWalletBackupScreen` in `lib/feature/login/impl`).
 *
 * The mnemonic phrase is passed inline through the NavKey rather than
 * read from `WalletRepository.unlock()` on the destination because at
 * this point the wallet has *just* been generated and the user has not
 * yet confirmed the backup - we don't want a BiometricPrompt before
 * the user has even seen the phrase to back up.
 *
 * The phrase is held in process memory only for the lifetime of this
 * NavBackStack entry; once the user taps "I have written it down" the
 * entry pops and the reference goes away.
 */
@Parcelize
data class WarrenWalletBackupNavKey(val mnemonicPhrase: String) : NavKey2
