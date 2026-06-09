package com.warrenbrowse.vpn.feature.login.impl

import androidx.lifecycle.ViewModel
import com.warrenbrowse.vpn.lib.model.wallet.Mnemonic
import com.warrenbrowse.vpn.lib.repository.MnemonicCache

/**
 * Scoped to the [com.warrenbrowse.vpn.feature.login.api.WarrenWalletBackupNavKey]
 * NavBackStackEntry: consumes the staged [Mnemonic] from
 * [MnemonicCache] once at construction time, holds it for the
 * lifetime of the back-stack entry, and closes it (zeroing its
 * [CharArray]) when [onCleared] fires.
 *
 * Audit follow-up: the previous design used
 * `remember { MnemonicCache.consume() }` directly inside the
 * Composable. `remember` is scoped to the composition, not to the
 * back-stack entry - a configuration change (rotation, dark-mode
 * toggle, ...) destroys and recomposes the Activity, ré-runs the
 * `remember` block, and the second `consume()` returns null because
 * the slot was already drained by the first composition. The user
 * would land on the "process restore failure" path despite never
 * leaving the app.
 *
 * A NavBackStackEntry-scoped ViewModel survives configuration
 * changes (the ViewModelStore is preserved across Activity recreate)
 * but is destroyed on process kill - which is exactly the lifecycle
 * we want for the mnemonic handoff slot.
 */
class WarrenWalletBackupViewModel : ViewModel() {

    /**
     * `null` when the cache slot was empty at ViewModel init -
     * typically after a process kill / restore. The screen routes
     * the user back to the login entry in that case.
     */
    val mnemonic: Mnemonic? = MnemonicCache.consume()

    override fun onCleared() {
        super.onCleared()
        // Zero the CharArray as soon as the NavBackStackEntry is
        // popped (user confirmed backup, or the host cleared the
        // back stack). Idempotent - safe if the screen never read it
        // OR if it was never staged.
        mnemonic?.close()
    }
}
