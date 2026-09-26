package com.warrenbrowse.vpn.app.connect

import com.warrenbrowse.vpn.lib.model.wallet.WalletState
import com.warrenbrowse.vpn.lib.repository.WalletRepository
import com.warrenbrowse.vpn.lib.repository.WarrenAccountStandingState
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged

/**
 * Redeems the vouchers a ban left waiting in the pending voucher store (warren-core doc 105
 * section 6): once when it starts and no ban is known (the app may have restarted during the
 * ban), again whenever a known ban stops holding, and for each wallet the device switches to.
 */
class HeldVoucherRedeemer(
    private val standing: WarrenAccountStandingState,
    private val wallet: WalletRepository,
    private val redeemHeld: suspend () -> Unit,
) {
    /** [run] while [wanted] is true: the privacy disclosure accepted, nothing leaves before it. */
    suspend fun runWhile(wanted: Flow<Boolean>) =
        wanted.distinctUntilChanged().collectLatest { if (it) run() }

    /** Runs until cancelled; the caller scopes it. */
    suspend fun run() =
        combine(standing.standing, wallet.state) { standing, wallet ->
                identityOf(wallet)?.let { it to (standing?.ban?.inForce == true) }
            }
            .distinctUntilChanged()
            .collect { key -> if (key != null && !key.second) redeemHeld() }

    private fun identityOf(state: WalletState): String? =
        when (state) {
            is WalletState.Locked -> state.pubkey.value
            is WalletState.Ready -> state.pubkey.value
            else -> null
        }
}
