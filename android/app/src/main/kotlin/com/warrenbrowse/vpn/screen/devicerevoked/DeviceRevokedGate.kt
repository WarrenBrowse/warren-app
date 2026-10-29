package com.warrenbrowse.vpn.screen.devicerevoked

import com.warrenbrowse.vpn.lib.model.DeviceState

/**
 * Whether the app must be taken to the revoked screen: another device of the account revoked this
 * one and the wallet already left it (warren-core doc 115), so whatever screen is up, whether the
 * connect screen, settings or the device list, belongs to an account this device no longer holds.
 */
internal fun showsRevokedScreen(state: DeviceState?, revokedScreenInStack: Boolean): Boolean =
    state == DeviceState.Revoked && !revokedScreenInStack
