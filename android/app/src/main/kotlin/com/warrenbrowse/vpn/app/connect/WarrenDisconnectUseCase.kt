package com.warrenbrowse.vpn.app.connect

import android.content.Context
import android.content.Intent
import co.touchlab.kermit.Logger
import com.warrenbrowse.vpn.app.service.WarrenVpnService
import com.warrenbrowse.vpn.lib.common.constant.KEY_DISCONNECT_ACTION
import com.warrenbrowse.vpn.lib.repository.WarrenQuinnDisconnectInvoker

/**
 * Sends [KEY_DISCONNECT_ACTION] to the running [WarrenVpnService]. The
 * service then routes the disconnect through the Quinn adapter.
 *
 * Held by Koin as a singleton so any caller (Connect button,
 * notification action, tile service) can issue the disconnect without
 * having to compose the Intent themselves.
 */
class WarrenDisconnectUseCase(
    private val context: Context,
) : WarrenQuinnDisconnectInvoker {

    override fun disconnect() {
        val intent = Intent(context, WarrenVpnService::class.java).apply {
            action = KEY_DISCONNECT_ACTION
        }
        try {
            context.startForegroundService(intent)
            Logger.i("WarrenDisconnectUseCase: dispatched disconnect intent")
        } catch (e: Exception) {
            Logger.e(throwable = e) { "WarrenDisconnectUseCase: dispatch failed" }
        }
    }
}
