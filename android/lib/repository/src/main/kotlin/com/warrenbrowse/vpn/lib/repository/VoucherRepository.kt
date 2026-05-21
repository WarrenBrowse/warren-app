package com.warrenbrowse.vpn.lib.repository

import com.warrenbrowse.vpn.lib.grpc.ManagementService
import com.warrenbrowse.vpn.lib.model.VoucherCode

class VoucherRepository(
    private val managementService: ManagementService,
    private val accountRepository: AccountRepository,
) {
    suspend fun submitVoucher(voucher: VoucherCode) =
        managementService.submitVoucher(voucher).onRight {
            accountRepository.onVoucherRedeemed(it.newExpiryDate)
        }
}
