package com.warrenbrowse.vpn.feature.login.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.home.api.ConnectNavKey
import com.warrenbrowse.vpn.feature.login.api.WarrenWalletBackupNavKey
import com.warrenbrowse.vpn.feature.login.api.WarrenWalletNavKey
import com.warrenbrowse.vpn.feature.login.impl.WarrenWalletBackupScreen
import com.warrenbrowse.vpn.feature.login.impl.WarrenWalletLoginScreen
import com.warrenbrowse.vpn.lib.repository.MnemonicCache

/**
 * NavGraph entries for the D.5 wallet onboarding flow.
 *
 * Routing contract:
 *   - [WarrenWalletNavKey] hosts the Generate/Restore branching screen.
 *     On generate, stashes the freshly-generated [com.warrenbrowse.vpn.lib.model.wallet.Mnemonic]
 *     in [MnemonicCache] (process-internal, NOT persisted to the
 *     saved-state Bundle) and pushes the [WarrenWalletBackupNavKey]
 *     sentinel. On restore (or any other transition into
 *     `WalletState.Ready`), pops to [ConnectNavKey] clearing the stack.
 *   - [WarrenWalletBackupNavKey] hosts the cleartext phrase + the
 *     "I have written it down" confirmation. On confirm, clears the
 *     back stack and pushes [ConnectNavKey].
 *
 * Audit follow-up: the previous implementation passed the phrase by
 * value through the NavKey parcelable, which Compose Navigation
 * persists in the saved-state Bundle — defeating the [Mnemonic]
 * zero-on-close lifecycle. The handoff now goes through the
 * [MnemonicCache] process-internal slot. A process kill empties the
 * cache, in which case [WarrenWalletBackupScreen] navigates back to
 * the login screen (the user has to re-trigger create-or-import
 * after a process death anyway).
 */
fun EntryProviderScope<NavKey2>.walletEntry(navigator: Navigator) {
    entry<WarrenWalletNavKey> {
        WarrenWalletLoginScreen(
            onWalletCreated = { mnemonic ->
                MnemonicCache.put(mnemonic)
                navigator.navigate(WarrenWalletBackupNavKey)
            },
            onWalletReady = {
                navigator.navigate(ConnectNavKey, clearBackStack = true)
            },
        )
    }

    entry<WarrenWalletBackupNavKey> {
        WarrenWalletBackupScreen(
            onConfirmed = {
                navigator.navigate(ConnectNavKey, clearBackStack = true)
            },
            onProcessRestoreFailure = {
                // The MnemonicCache slot was empty (process kill or
                // out-of-band drain). Navigate back to the wallet
                // login screen so the user can re-trigger
                // create-or-import.
                navigator.navigate(WarrenWalletNavKey, clearBackStack = true)
            },
        )
    }
}
