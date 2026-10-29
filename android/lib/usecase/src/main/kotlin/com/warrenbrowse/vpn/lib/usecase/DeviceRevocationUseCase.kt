package com.warrenbrowse.vpn.lib.usecase

import arrow.core.Either
import co.touchlab.kermit.Logger
import com.warrenbrowse.vpn.lib.model.DeviceRegistryError
import com.warrenbrowse.vpn.lib.model.wallet.WalletState
import com.warrenbrowse.vpn.lib.repository.DeviceRepository
import com.warrenbrowse.vpn.lib.repository.WalletRepository
import kotlin.time.Duration
import kotlin.time.Duration.Companion.minutes
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.merge
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock

/**
 * Acts on this device's revocation from another device of the account (warren-core doc 115,
 * decision 1): the tunnel comes down, the wallet leaves the device, and the app shows its revoked
 * screen. Coming back takes the recovery phrase.
 *
 * The native side learns the revocation on its own (a token or entitlement refresh, a device
 * registry call, the registration at a dial) and ends a running session on it; this reads the flag
 * it raises, which costs an atomic load and no request.
 */
class DeviceRevocationUseCase(
    private val devices: DeviceRepository,
    private val wallet: WalletRepository,
    private val logOut: suspend () -> WarrenLogoutOutcome,
) {
    private val acting = Mutex()

    /**
     * Takes a revocation the registry reported in, if there is one. A logout that did not complete
     * (the tunnel did not come down in time, the erase failed) leaves the flag up, so the next call
     * tries again.
     */
    suspend fun takeInRevocation() =
        acting.withLock {
            if (!devices.revokedByRegistry()) return@withLock
            // A revocation learned after the wallet left concerns a wallet already gone.
            if (wallet.state.value is WalletState.Absent) {
                devices.dropRevocation()
                return@withLock
            }
            when (val outcome = logOut()) {
                WarrenLogoutOutcome.LoggedOut -> devices.showRevoked()
                else ->
                    Logger.w {
                        "revoked device: logout did not complete (${outcome::class.simpleName})"
                    }
            }
        }

    /**
     * "Log out" on this device's row of the device list: the registry revokes it, then the wallet
     * leaves the device as on any logout. Held against [takeInRevocation], so the revocation this
     * raises natively is this logout's and never shows the revoked screen.
     */
    suspend fun logOutThisDevice(deviceId: String): Either<DeviceRegistryError, WarrenLogoutOutcome> =
        acting.withLock {
            devices.revokeDevice(deviceId).map {
                val outcome = logOut()
                devices.dropRevocation()
                outcome
            }
        }

    /**
     * Calls [takeInRevocation] on every [wakes] emission (a tunnel transition, which is how a
     * session the native side ended on the revocation is noticed at once) and every [period], for
     * a refresh that learned it while no tunnel ran. Runs until cancelled.
     */
    suspend fun watch(wakes: Flow<Any?>, period: Duration = PERIOD) {
        merge(wakes, ticks(period)).collect { takeInRevocation() }
    }

    private fun ticks(period: Duration): Flow<Unit> = flow {
        while (true) {
            emit(Unit)
            delay(period)
        }
    }

    companion object {
        /** Well under the ten minute refresh round that learns most revocations. */
        val PERIOD: Duration = 1.minutes
    }
}
