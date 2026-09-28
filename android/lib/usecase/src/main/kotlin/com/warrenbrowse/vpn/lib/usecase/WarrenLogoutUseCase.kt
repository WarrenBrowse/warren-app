package com.warrenbrowse.vpn.lib.usecase

import com.warrenbrowse.vpn.lib.repository.WalletRepository
import com.warrenbrowse.vpn.lib.repository.WarrenConnectedInfo
import com.warrenbrowse.vpn.lib.repository.WarrenQuinnDisconnectInvoker
import com.warrenbrowse.vpn.lib.repository.WarrenTunnelStateProvider
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.withTimeoutOrNull

sealed interface WarrenLogoutOutcome {
    data object LoggedOut : WarrenLogoutOutcome

    /**
     * The tunnel teardown did not land within the bound, so the wallet was left in place. The
     * disconnect stays queued behind whatever held the tunnel, and a retry logs out once it lands.
     */
    data object TunnelStillUp : WarrenLogoutOutcome

    data class EraseFailed(val cause: Throwable) : WarrenLogoutOutcome
}

/**
 * Logging out removes the wallet from the device, and a logged-out app has no tunnel: the session,
 * its per-app routes and forwarded ports, and the kill-switch blackhole all belong to the wallet
 * that leaves. So the tunnel comes down first and the wallet is erased only once it is down. The
 * reverse order is how beta 1.1.38 showed the login screen over a tunnel still carrying traffic,
 * with nothing left in the app able to stop it.
 */
class WarrenLogoutUseCase(
    private val tunnelState: WarrenTunnelStateProvider,
    private val disconnect: WarrenQuinnDisconnectInvoker,
    private val wallet: WalletRepository,
    private val teardownTimeoutMs: Long = DEFAULT_TEARDOWN_TIMEOUT_MS,
) {
    suspend operator fun invoke(): WarrenLogoutOutcome {
        // A disconnect dispatch starts the tunnel service when nothing runs,
        // so it is only sent when there is something to take down.
        if (tunnelState.connectedInfo.value !is WarrenConnectedInfo.Disconnected) {
            disconnect.disconnect()
            withTimeoutOrNull(teardownTimeoutMs) {
                tunnelState.connectedInfo.first { it is WarrenConnectedInfo.Disconnected }
            } ?: return WarrenLogoutOutcome.TunnelStillUp
        }
        return runCatching { wallet.erase() }
            .fold({ WarrenLogoutOutcome.LoggedOut }, { WarrenLogoutOutcome.EraseFailed(it) })
    }

    companion object {
        /**
         * The native teardown takes tens of milliseconds, but the adapter runs it only once any
         * dial in flight releases the tunnel lock, and a dial on a cold runtime can hold it for
         * seconds.
         */
        const val DEFAULT_TEARDOWN_TIMEOUT_MS = 15_000L
    }
}
