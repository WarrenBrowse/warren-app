package com.warrenbrowse.vpn.lib.billing.extension

import com.android.billingclient.api.ProductDetails
import com.warrenbrowse.vpn.lib.payment.model.PaymentProduct
import com.warrenbrowse.vpn.lib.payment.model.PaymentStatus
import com.warrenbrowse.vpn.lib.payment.model.ProductId
import com.warrenbrowse.vpn.lib.payment.model.ProductPrice

fun ProductDetails.toPaymentProduct(productIdToStatus: Map<String, PaymentStatus?>) =
    PaymentProduct(
        productId = ProductId(this.productId),
        price = ProductPrice(this.oneTimePurchaseOfferDetails?.formattedPrice ?: ""),
        productIdToStatus[this.productId],
    )

fun List<ProductDetails>.toPaymentProducts(productIdToStatus: Map<String, PaymentStatus?>) =
    this.map { it.toPaymentProduct(productIdToStatus) }
