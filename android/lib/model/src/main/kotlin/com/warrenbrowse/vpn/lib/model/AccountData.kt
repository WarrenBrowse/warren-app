package com.warrenbrowse.vpn.lib.model

import java.time.ZonedDateTime

data class AccountData(
    val id: AccountId,
    val accountNumber: AccountNumber,
    val expiryDate: ZonedDateTime,
) {
    companion object
}
