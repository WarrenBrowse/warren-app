package com.warrenbrowse.vpn.feature.login.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.core.animation.wizardForwardTransition
import com.warrenbrowse.vpn.feature.home.api.ConnectNavKey
import com.warrenbrowse.vpn.feature.login.api.WarrenKeysNavKey
import com.warrenbrowse.vpn.feature.login.api.WarrenRestoreMnemonicNavKey
import com.warrenbrowse.vpn.feature.login.api.WarrenWalletBackupNavKey
import com.warrenbrowse.vpn.feature.login.api.WarrenWalletNavKey
import com.warrenbrowse.vpn.feature.login.impl.WarrenKeysScreen
import com.warrenbrowse.vpn.feature.login.impl.WarrenRestoreMnemonicScreen
import com.warrenbrowse.vpn.feature.login.impl.WarrenWalletBackupScreen
import com.warrenbrowse.vpn.feature.login.impl.WarrenWalletLoginScreen
import com.warrenbrowse.vpn.feature.settings.api.SettingsNavKey
import com.warrenbrowse.vpn.lib.repository.MnemonicCache

/**
 * NavGraph entries for the wallet onboarding flow.
 *
 * Routing contract:
 *   - [WarrenWalletNavKey] hosts the Generate/Restore branching screen.
 *     On generate, stashes the freshly-generated [com.warrenbrowse.vpn.lib.model.wallet.Mnemonic]
 *     in [MnemonicCache] (process-internal, NOT persisted to the
 *     saved-state Bundle) and pushes the [WarrenWalletBackupNavKey]
 *     sentinel. On restore (or any other transition into
 *     `WalletState.Ready`), pops to [ConnectNavKey] clearing the stack.
 *   - [WarrenWalletBackupNavKey] hosts the cleartext phrase + the backup
 *     confirmation. It is a hard gate with no way back: the wallet is already
 *     persisted, so leaving would strand an identity nobody has written down.
 *     On confirm, clears the back stack and pushes [ConnectNavKey].
 *   - [WarrenKeysNavKey] hosts the same phrase read back later from the account
 *     page, and [WarrenRestoreMnemonicNavKey] the non-destructive switch to
 *     another wallet, both mirroring the desktop keys / restore-keys routes.
 *
 * Audit follow-up: the previous implementation passed the phrase by
 * value through the NavKey parcelable, which Compose Navigation
 * persists in the saved-state Bundle - defeating the [Mnemonic]
 * zero-on-close lifecycle. The handoff now goes through the
 * [MnemonicCache] process-internal slot. A process kill empties the
 * cache, in which case [WarrenWalletBackupScreen] navigates back to
 * the login screen (the user has to re-trigger create-or-import
 * after a process death anyway).
 */
fun EntryProviderScope<NavKey2>.walletEntry(
    navigator: Navigator,
    // Host-supplied post-wallet destination. Defaults to Connect; on first-run
    // onboarding the app passes the wizard's first step. Kept as a callback so
    // this module does not depend on the app-module onboarding NavKeys.
    postWalletDestination: (onboarding: Boolean) -> NavKey2 = { ConnectNavKey },
    // True when the login screen is being reached by a root swap rather than by a
    // wizard step. Supplied by the host for the same reason as above: only it knows
    // which of its own keys are roots.
    reachedByRootSwap: () -> Boolean = { false },
) {
    entry<WarrenWalletNavKey>(metadata = wizardForwardTransition(reachedByRootSwap)) { key ->
        WarrenWalletLoginScreen(
            onWalletCreated = { mnemonic ->
                MnemonicCache.put(mnemonic)
                navigator.navigate(WarrenWalletBackupNavKey(onboarding = key.onboarding))
            },
            onWalletReady = {
                navigator.navigate(postWalletDestination(key.onboarding), clearBackStack = true)
            },
            // Desktop keeps the settings escape on the pick and restore steps
            // and disables it only while the backup gate is up.
            onSettingsClicked = { navigator.navigate(SettingsNavKey) },
        )
    }

    entry<WarrenWalletBackupNavKey>(metadata = wizardForwardTransition()) { key ->
        WarrenWalletBackupScreen(
            onConfirmed = {
                navigator.navigate(postWalletDestination(key.onboarding), clearBackStack = true)
            },
            onProcessRestoreFailure = {
                // The MnemonicCache slot was empty (process kill or
                // out-of-band drain). Navigate back to the wallet
                // login screen so the user can re-trigger
                // create-or-import.
                navigator.navigate(WarrenWalletNavKey(), clearBackStack = true)
            },
        )
    }

    entry<WarrenKeysNavKey> {
        WarrenKeysScreen(
            onDone = { navigator.goBack() },
            onRestoreOtherPhrase = { navigator.navigate(WarrenRestoreMnemonicNavKey) },
            onProcessRestoreFailure = { navigator.goBack() },
        )
    }

    entry<WarrenRestoreMnemonicNavKey> {
        WarrenRestoreMnemonicScreen(
            // The restored wallet is a different identity: send the user back to
            // the main flow rather than to the keys screen, whose staged phrase
            // belongs to the wallet that was just replaced.
            onRestored = {
                navigator.navigate(postWalletDestination(false), clearBackStack = true)
            },
            onNavigateBack = { navigator.goBack() },
        )
    }
}
