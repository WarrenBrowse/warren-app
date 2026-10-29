package com.warrenbrowse.vpn.lib.repository

import arrow.core.Either
import arrow.core.left
import arrow.core.right
import co.touchlab.kermit.Logger
import com.warrenbrowse.vpn.lib.model.AccountDevice
import com.warrenbrowse.vpn.lib.model.AccountDeviceList
import com.warrenbrowse.vpn.lib.model.DeviceRegistryError
import com.warrenbrowse.vpn.lib.model.DeviceState
import java.time.LocalDate
import java.time.format.DateTimeParseException
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.withContext
import kotlinx.serialization.SerializationException
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.contentOrNull

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
 * The account's devices, signed with the wallet on the device, and the login state as far as the
 * device registry speaks to it: [DeviceState.Revoked] once this device was revoked from another
 * device of the account and the wallet has left it, which takes the app to its revoked screen.
 * Nothing (`null`) otherwise: the wallet login itself is the account repositories' to tell.
 */
class DeviceRepository(
    private val bridge: WarrenDeviceBridge? = null,
    private val wallet: WalletRepository? = null,
    private val io: CoroutineDispatcher = Dispatchers.IO,
) {
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

    /** The account's devices, this one marked. */
    suspend fun listDevices(): Either<DeviceRegistryError, AccountDeviceList> =
        call({ bridge, phrase -> bridge.listDevices(phrase) }) { envelope ->
            AccountDeviceList(
                devices =
                    (envelope["devices"] as? JsonArray).orEmpty().mapNotNull {
                        (it as? JsonObject)?.let(::deviceOf)
                    },
                olderClientSeenOn = dayOf(envelope.string("older_client_seen_on")),
            )
        }

    /** Renames the account's device [deviceId]; answers whether it is this device. */
    suspend fun renameDevice(deviceId: String, name: String): Either<DeviceRegistryError, Boolean> =
        call({ bridge, phrase -> bridge.renameDevice(phrase, deviceId, name) }) {
            it.bool("this_device") ?: false
        }

    /**
     * Revokes the account's device [deviceId]; answers whether it is this device, which the
     * registry then reads as revoked.
     */
    suspend fun revokeDevice(deviceId: String): Either<DeviceRegistryError, Boolean> =
        call({ bridge, phrase -> bridge.revokeDevice(phrase, deviceId) }) {
            it.bool("this_device") ?: false
        }

    // The Keystore read and the JNI call are system boundaries: whatever crosses them as a
    // throwable is a call that did not happen, never a crash.
    @Suppress("TooGenericExceptionCaught")
    private suspend fun <T> call(
        request: (WarrenDeviceBridge, String) -> String,
        answer: (JsonObject) -> T,
    ): Either<DeviceRegistryError, T> =
        withContext(io) {
            val bridge = bridge ?: return@withContext DeviceRegistryError.NoWallet.left()
            val mnemonic =
                try {
                    wallet?.readMnemonic()
                } catch (e: Exception) {
                    Logger.w { "device registry: wallet unreadable (${e::class.simpleName})" }
                    null
                } ?: return@withContext DeviceRegistryError.NoWallet.left()
            val raw =
                try {
                    mnemonic.use { m -> m.useAsString { request(bridge, it) } }
                } catch (e: Exception) {
                    Logger.w { "device registry call threw (${e::class.simpleName})" }
                    return@withContext DeviceRegistryError.Unavailable.left()
                }
            val envelope = parse(raw) ?: return@withContext DeviceRegistryError.Unavailable.left()
            if (envelope.bool("ok") == true) {
                answer(envelope).right()
            } else {
                DeviceRegistryError.fromCode(envelope.string("error")).left()
            }
        }

    // The decoder's message is never logged: it quotes the input, which names the account's
    // devices.
    private fun parse(raw: String): JsonObject? =
        try {
            Json.parseToJsonElement(raw) as? JsonObject
        } catch (e: SerializationException) {
            Logger.w { "device registry answered a malformed envelope (${e::class.simpleName})" }
            null
        }

    private fun deviceOf(row: JsonObject): AccountDevice? {
        val id = row.string("device_id") ?: return null
        return AccountDevice(
            id = id,
            name = row.string("name").orEmpty(),
            platform = row.string("platform").orEmpty(),
            appVersion = row.string("app_version").orEmpty(),
            lastSeenOn = dayOf(row.string("last_seen_on")),
            thisDevice = row.bool("this_device") ?: false,
        )
    }

    private fun dayOf(day: String?): LocalDate? =
        day?.let {
            try {
                LocalDate.parse(it)
            } catch (e: DateTimeParseException) {
                null
            }
        }
}

private fun JsonObject.string(key: String): String? = (this[key] as? JsonPrimitive)?.contentOrNull

private fun JsonObject.bool(key: String): Boolean? = (this[key] as? JsonPrimitive)?.booleanOrNull
