package com.warrenbrowse.vpn.lib.repository

import com.warrenbrowse.vpn.lib.model.wallet.WalletState
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch

/**
 * Clears the forum identity when the wallet leaves the device. The handle is
 * account data: public on the forum, but its link to this device is not, and
 * the next wallet set up here must not inherit it (the desktop clears its forum
 * store on logout). Bound to the wallet state rather than to one erase call
 * site, so every path that removes the wallet is covered, and an identity left
 * behind by an erase that predates this binding goes at the next start.
 */
class ForumIdentityWalletBinding(
    private val wallet: WalletRepository,
    private val forumIdentity: ForumIdentityRepository,
    private val scope: CoroutineScope,
) {
    fun start(): Job =
        scope.launch {
            wallet.state.collect { state ->
                if (state is WalletState.Absent && forumIdentity.identity.value != null) {
                    forumIdentity.clear()
                }
            }
        }
}
