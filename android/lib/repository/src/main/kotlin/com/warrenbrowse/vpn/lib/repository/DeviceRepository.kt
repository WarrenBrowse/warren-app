package com.warrenbrowse.vpn.lib.repository

import com.warrenbrowse.vpn.lib.model.DeviceState
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/**
 * The native calls on the account's device registry (warren-core doc 115,
 * `warren-jni` `devices` module). Kept apart from [WarrenJniBridge] so a
 * consumer and its fakes carry only these.
 *
 * The list, rename and revoke calls answer JSON envelopes: `{"ok":true,..}` or
 * `{"ok":false,"error":"no_wallet|invalid_device|invalid_name|not_found|revoked|unavailable"}`.
 * They block on a network call: invoke them off the main thread. A listed device
 * names an install of the account and the days it was used: never log one.
 */
interface WarrenDeviceBridge {
    /**
     * The account's devices, this one marked:
     * `{"ok":true,"devices":[{"device_id","name","platform","app_version","registered_on",
     * "last_seen_on","this_device"}..],"older_client_seen_on":"YYYY-MM-DD"|null}`.
     */
    fun listDevices(mnemonic: String): String

    /** Renames the account's device [deviceId]: `{"ok":true,"this_device":bool}`. */
    fun renameDevice(mnemonic: String, deviceId: String, name: String): String

    /**
     * Revokes the account's device [deviceId]: `{"ok":true,"this_device":bool}`. Revoking this
     * device is a logout, after which [deviceRevoked] reads `true`.
     */
    fun revokeDevice(mnemonic: String, deviceId: String): String

    /**
     * Whether the registry revoked this device since [forgetDeviceRevocation]: its token and
     * entitlement refreshes have stopped and its device identity is forgotten.
     */
    fun deviceRevoked(): Boolean

    /** The revocation was taken in (the user left the revoked screen, or logged in again). */
    fun forgetDeviceRevocation()
}

/**
 * The login state as far as the device registry speaks to it: [DeviceState.Revoked] once this
 * device was revoked from another device of the account and the wallet has left it, which takes
 * the app to its revoked screen. Nothing (`null`) otherwise: the wallet login itself is the
 * account repositories' to tell.
 */
class DeviceRepository(private val bridge: WarrenDeviceBridge? = null) {
    private val _deviceState = MutableStateFlow<DeviceState?>(null)
    val deviceState: StateFlow<DeviceState?> = _deviceState.asStateFlow()

    /** Whether the registry revoked this device since the app last took a revocation in. */
    fun revokedByRegistry(): Boolean = bridge?.deviceRevoked() == true

    /**
     * Takes the registry's revocation in: the wallet has left the device, and the revoked screen
     * says why. The native flag is consumed, so the same revocation is acted on once.
     */
    fun showRevoked() {
        bridge?.forgetDeviceRevocation()
        _deviceState.value = DeviceState.Revoked
    }

    /** Drops a revocation that concerns no wallet any more, without a screen. */
    fun dropRevocation() {
        bridge?.forgetDeviceRevocation()
    }

    /** The user left the revoked screen: the next login starts from a clean state. */
    fun acknowledgeRevocation() {
        bridge?.forgetDeviceRevocation()
        _deviceState.value = null
    }
}
