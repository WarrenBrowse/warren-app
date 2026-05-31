package com.warrenbrowse.vpn.lib.model

import android.os.Parcelable
import kotlinx.parcelize.Parcelize

// Warren login-state notion. The per-device payload was removed: Warren's
// identity is the BIP39 wallet (account/pubkey), with no per-"device"
// abstraction or WireGuard key material. `LoggedIn` carries only the
// account identity; `Revoked` keeps the recovery-to-login path.
sealed class DeviceState : Parcelable {
    @Parcelize
    data class LoggedIn(val accountNumber: AccountNumber) : DeviceState()

    @Parcelize data object LoggedOut : DeviceState()

    @Parcelize data object Revoked : DeviceState()

    fun accountNumber(): AccountNumber? {
        return (this as? LoggedIn)?.accountNumber
    }
}
