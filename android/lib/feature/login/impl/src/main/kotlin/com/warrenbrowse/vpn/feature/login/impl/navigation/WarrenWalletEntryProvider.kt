package com.warrenbrowse.vpn.feature.login.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.home.api.ConnectNavKey
import com.warrenbrowse.vpn.feature.login.api.WarrenWalletBackupNavKey
import com.warrenbrowse.vpn.feature.login.api.WarrenWalletNavKey
import com.warrenbrowse.vpn.feature.login.impl.WarrenWalletBackupScreen
import com.warrenbrowse.vpn.feature.login.impl.WarrenWalletLoginScreen
import com.warrenbrowse.vpn.lib.model.wallet.Mnemonic

/**
 * NavGraph entries for the D.5 wallet onboarding flow.
 *
 * Routing contract:
 *   - [WarrenWalletNavKey] hosts the Generate/Restore branching screen.
 *     On generate, pushes [WarrenWalletBackupNavKey] with the freshly
 *     generated phrase. On restore (or any other transition into
 *     `WalletState.Ready`), pops to [ConnectNavKey] clearing the stack.
 *   - [WarrenWalletBackupNavKey] hosts the cleartext phrase + the
 *     "I have written it down" confirmation. On confirm, clears the
 *     back stack and pushes [ConnectNavKey].
 *
 * The phrase is passed by value through the NavKey (parcelable) rather
 * than read from `WalletRepository.unlock()` because at this point the
 * mnemonic has *just* been generated and there is no point gating the
 * very first read of it behind a biometric prompt - the user has not
 * even seen the phrase yet.
 */
fun EntryProviderScope<NavKey2>.walletEntry(navigator: Navigator) {
    entry<WarrenWalletNavKey> {
        WarrenWalletLoginScreen(
            onWalletCreated = { mnemonic ->
                navigator.navigate(WarrenWalletBackupNavKey(mnemonic.phrase))
            },
            onWalletReady = {
                navigator.navigate(ConnectNavKey, clearBackStack = true)
            },
        )
    }

    entry<WarrenWalletBackupNavKey> { navKey ->
        WarrenWalletBackupScreen(
            mnemonic = Mnemonic(navKey.mnemonicPhrase),
            onConfirmed = {
                navigator.navigate(ConnectNavKey, clearBackStack = true)
            },
        )
    }
}
