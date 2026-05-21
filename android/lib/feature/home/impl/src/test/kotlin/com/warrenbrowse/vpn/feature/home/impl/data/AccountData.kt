package com.warrenbrowse.vpn.feature.home.impl.data

import io.mockk.mockk
import java.time.ZonedDateTime
import com.warrenbrowse.vpn.lib.model.AccountData

fun AccountData.Companion.mock(expiry: ZonedDateTime): AccountData =
    AccountData(
        id = mockk(relaxed = true),
        accountNumber = mockk(relaxed = true),
        expiryDate = expiry,
    )
