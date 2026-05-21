package com.warrenbrowse.vpn.lib.repository

import androidx.fragment.app.FragmentActivity
import kotlinx.coroutines.flow.StateFlow

/**
 * Lib-side surface for the Warren Connect orchestrator. The concrete
 * implementation lives in `app/connect/WarrenConnectUseCase` and is
 * bound to this interface in `di/AppModule`. The interface lives here
 * (in `lib/repository`, alongside [WalletRepository]) so any feature
 * module - `lib/feature/home/impl`, `lib/feature/settings/impl`, etc. -
 * can consume the surface without depending on the `app` module
 * (forbidden dependency arrow).
 */
interface WarrenQuinnConnectInvoker {
    /**
     * Authenticate, build config, stash mnemonic, dispatch the Quinn
     * connect intent. Returns a human-readable status string suitable
     * for inline display.
     */
    suspend fun connect(activity: FragmentActivity): String
}

/**
 * Lib-side surface for the live Warren tunnel state. The concrete
 * impl is `app/service/WarrenQuinnStateProxy` which mirrors the
 * service-owned [com.warrenbrowse.vpn.app.service.WarrenQuinnAdapter.state]
 * into a process-wide StateFlow. Consumers in feature modules subscribe
 * here (with a `String` projection so they don't need to import the
 * app-private `WarrenTunnelState` sealed type).
 */
interface WarrenTunnelStateProvider {
    val state: StateFlow<String>
}

/**
 * Lib-side surface for the Warren disconnect path. The concrete impl
 * lives in `app/connect/WarrenDisconnectUseCase` and is bound to this
 * interface in `di/AppModule`. The disconnect path does not need
 * biometric authorisation (it tears down a running session); a plain
 * [android.content.Context] is sufficient because no UI dialog is
 * raised.
 */
interface WarrenQuinnDisconnectInvoker {
    fun disconnect()
}
