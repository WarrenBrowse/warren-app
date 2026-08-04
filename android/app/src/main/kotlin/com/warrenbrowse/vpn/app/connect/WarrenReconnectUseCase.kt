package com.warrenbrowse.vpn.app.connect

import android.content.Context
import android.content.Intent
import co.touchlab.kermit.Logger
import com.warrenbrowse.vpn.app.service.WarrenVpnService
import com.warrenbrowse.vpn.lib.common.constant.KEY_RECONNECT_ACTION
import com.warrenbrowse.vpn.lib.repository.ConnectionProxy
import com.warrenbrowse.vpn.lib.repository.WarrenQuinnReconnectInvoker

/**
 * Dispatches [KEY_RECONNECT_ACTION] to the running [WarrenVpnService].
 *
 * The service rebuilds the tunnel config from the CURRENT settings (off the
 * main thread) and reconnects applying it, so a reconnect picks up changes made
 * since the last connect (exit relay, entry country, DAITA, NAT-PMP, DNS, ...).
 * The mnemonic is not needed here: the adapter still holds it from the live
 * session, so there is no biometric re-prompt.
 *
 * No-op if no session is running (the user has to use the normal connect flow
 * instead). Equivalent to a quick connect/disconnect cycle.
 */
class WarrenReconnectUseCase(
    private val context: Context,
    private val connectionProxy: ConnectionProxy,
) : WarrenQuinnReconnectInvoker {

    override fun reconnect() {
        val intent = Intent(context, WarrenVpnService::class.java).apply {
            action = KEY_RECONNECT_ACTION
        }
        // The teardown passes through Disconnected before the next dial: tell
        // the card a re-dial is under way so it presents one continuous
        // transition instead of dropping to the disconnected screen mid-switch.
        connectionProxy.onReconnectRequested()
        try {
            context.startForegroundService(intent)
            Logger.i("WarrenReconnectUseCase: dispatched reconnect intent")
        } catch (e: Exception) {
            connectionProxy.onRequestAborted()
            Logger.e(throwable = e) { "WarrenReconnectUseCase: dispatch failed" }
        }
    }
}
