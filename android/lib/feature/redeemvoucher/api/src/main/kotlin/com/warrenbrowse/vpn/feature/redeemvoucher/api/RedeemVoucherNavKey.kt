package com.warrenbrowse.vpn.feature.redeemvoucher.api

import kotlinx.parcelize.Parcelize
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.NavResult

@Parcelize object RedeemVoucherNavKey : NavKey2

@Parcelize data class RedeemVoucherNavResult(val isTimeAdded: Boolean) : NavResult
