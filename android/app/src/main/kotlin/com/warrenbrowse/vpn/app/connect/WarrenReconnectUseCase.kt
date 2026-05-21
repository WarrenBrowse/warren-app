package com.warrenbrowse.vpn.app.connect

import android.content.Context
import android.content.Intent
import co.touchlab.kermit.Logger
import com.warrenbrowse.vpn.app.service.WarrenVpnService
import com.warrenbrowse.vpn.lib.common.constant.KEY_RECONNECT_ACTION
import com.warrenbrowse.vpn.lib.repository.WarrenQuinnReconnectInvoker

/**
 * Dispatches [KEY_RECONNECT_ACTION] to the running [WarrenVpnService].
 * The service routes the reconnect through the Quinn adapter, which
 * reuses the cached [WarrenTunnelConfig] + mnemonic - no biometric
 * re-prompt.
 *
 * No-op if no session is running (the user has to use the normal
 * connect flow instead). Equivalent to a quick connect/disconnect
 * cycle.
 */
class WarrenReconnectUseCase(
    private val context: Context,
) : WarrenQuinnReconnectInvoker {

    override fun reconnect() {
        val intent = Intent(context, WarrenVpnService::class.java).apply {
            action = KEY_RECONNECT_ACTION
        }
        try {
            context.startForegroundService(intent)
            Logger.i("WarrenReconnectUseCase: dispatched reconnect intent")
        } catch (e: Exception) {
            Logger.e(throwable = e) { "WarrenReconnectUseCase: dispatch failed" }
        }
    }
}
