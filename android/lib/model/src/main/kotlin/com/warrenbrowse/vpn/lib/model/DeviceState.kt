package com.warrenbrowse.vpn.lib.model

import android.os.Parcelable
import kotlinx.parcelize.Parcelize

// Account login state. Warren's identity is the BIP39 wallet
// (account/pubkey); there is no per-"device" abstraction. `LoggedIn`
// carries the account identity; `Revoked` keeps the recovery-to-login path.
sealed class DeviceState : Parcelable {
    @Parcelize
    data class LoggedIn(val accountNumber: AccountNumber) : DeviceState()

    @Parcelize data object LoggedOut : DeviceState()

    @Parcelize data object Revoked : DeviceState()

    fun accountNumber(): AccountNumber? {
        return (this as? LoggedIn)?.accountNumber
    }
}
