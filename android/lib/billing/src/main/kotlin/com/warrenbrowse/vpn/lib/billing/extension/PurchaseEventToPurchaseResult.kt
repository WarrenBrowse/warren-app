package com.warrenbrowse.vpn.lib.billing.extension

import com.warrenbrowse.vpn.lib.billing.model.PurchaseEvent
import com.warrenbrowse.vpn.lib.payment.model.PurchaseResult

fun PurchaseEvent.toPurchaseResult() =
    when (this) {
        is PurchaseEvent.Error -> PurchaseResult.Error.BillingError(this.exception)
        is PurchaseEvent.Completed -> PurchaseResult.VerificationStarted
        PurchaseEvent.UserCanceled -> PurchaseResult.Completed.Cancelled
    }
