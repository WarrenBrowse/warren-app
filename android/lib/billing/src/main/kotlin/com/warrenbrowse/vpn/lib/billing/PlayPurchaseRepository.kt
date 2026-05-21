package com.warrenbrowse.vpn.lib.billing

import com.warrenbrowse.vpn.lib.grpc.ManagementService
import com.warrenbrowse.vpn.lib.model.PlayPurchase

class PlayPurchaseRepository(private val managementService: ManagementService) {
    suspend fun initializePlayPurchase() = managementService.initializePlayPurchase()

    suspend fun verifyPlayPurchase(purchase: PlayPurchase) =
        managementService.verifyPlayPurchase(purchase)
}
