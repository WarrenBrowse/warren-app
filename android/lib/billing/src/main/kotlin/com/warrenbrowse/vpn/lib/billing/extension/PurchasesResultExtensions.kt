package com.warrenbrowse.vpn.lib.billing.extension

import com.android.billingclient.api.Purchase
import com.android.billingclient.api.PurchasesResult
import com.warrenbrowse.vpn.lib.billing.model.BillingException

fun PurchasesResult.nonPendingPurchases(): List<Purchase> =
    this.purchasesList.filter { it.purchaseState != Purchase.PurchaseState.PENDING }

fun PurchasesResult.responseCode(): Int = this.billingResult.responseCode

fun PurchasesResult.toBillingException(): BillingException =
    BillingException(responseCode = this.responseCode(), message = this.billingResult.debugMessage)
