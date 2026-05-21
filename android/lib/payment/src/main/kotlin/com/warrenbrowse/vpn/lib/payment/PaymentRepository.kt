package com.warrenbrowse.vpn.lib.payment

import android.app.Activity
import arrow.core.Either
import kotlinx.coroutines.flow.Flow
import com.warrenbrowse.vpn.lib.payment.model.PaymentAvailability
import com.warrenbrowse.vpn.lib.payment.model.ProductId
import com.warrenbrowse.vpn.lib.payment.model.PurchaseResult
import com.warrenbrowse.vpn.lib.payment.model.VerificationError
import com.warrenbrowse.vpn.lib.payment.model.VerificationResult

interface PaymentRepository {

    fun purchaseProduct(
        productId: ProductId,
        activityProvider: () -> Activity,
    ): Flow<PurchaseResult>

    suspend fun verifyPurchases(): Either<VerificationError, VerificationResult>

    fun queryPaymentAvailability(): Flow<PaymentAvailability>
}
