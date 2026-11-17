package com.warrenbrowse.vpn.lib.usecase.inappnotification

import com.warrenbrowse.vpn.lib.model.InAppNotification
import com.warrenbrowse.vpn.lib.model.wallet.WalletState
import com.warrenbrowse.vpn.lib.repository.DeviceRepository
import com.warrenbrowse.vpn.lib.repository.WalletRepository
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged

// The API refused this device as disconnected without the account wallet's
// signature over that (desktop WarrenRevocationUnconfirmedNotificationProvider
// parity): the session ended and the wallet stayed. Shown while that wallet
// is on the device.
class DeviceRevocationUnconfirmedNotificationUseCase(
    private val devices: DeviceRepository,
    private val wallet: WalletRepository,
) : InAppNotificationUseCase {
    override operator fun invoke(): Flow<InAppNotification?> =
        combine(devices.revocationUnconfirmed, wallet.state) { unconfirmed, state ->
                if (unconfirmed && state !is WalletState.Absent) {
                    InAppNotification.DeviceRevocationUnconfirmed
                } else {
                    null
                }
            }
            .distinctUntilChanged()
}
