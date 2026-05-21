package com.warrenbrowse.vpn.lib.billing

import android.app.Activity
import arrow.core.Either
import arrow.core.flatMap
import arrow.core.left
import arrow.core.raise.either
import arrow.core.raise.ensure
import arrow.core.right
import co.touchlab.kermit.Logger
import com.android.billingclient.api.BillingClient.BillingResponseCode
import com.android.billingclient.api.Purchase
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.firstOrNull
import kotlinx.coroutines.flow.flow
import com.warrenbrowse.vpn.lib.billing.extension.getProductDetails
import com.warrenbrowse.vpn.lib.billing.extension.nonPendingPurchases
import com.warrenbrowse.vpn.lib.billing.extension.responseCode
import com.warrenbrowse.vpn.lib.billing.extension.toBillingException
import com.warrenbrowse.vpn.lib.billing.extension.toPaymentAvailability
import com.warrenbrowse.vpn.lib.billing.extension.toPaymentStatus
import com.warrenbrowse.vpn.lib.billing.extension.toPurchaseResult
import com.warrenbrowse.vpn.lib.billing.extension.toPurchaseResultError
import com.warrenbrowse.vpn.lib.billing.extension.toPurchaseVerificationError
import com.warrenbrowse.vpn.lib.billing.model.BillingException
import com.warrenbrowse.vpn.lib.billing.model.PurchaseEvent
import com.warrenbrowse.vpn.lib.model.PlayExternalObfuscatedAccountId
import com.warrenbrowse.vpn.lib.model.PlayPurchase
import com.warrenbrowse.vpn.lib.model.PlayPurchaseInitError
import com.warrenbrowse.vpn.lib.model.PlayPurchasePaymentToken
import com.warrenbrowse.vpn.lib.model.PlayPurchaseVerifyError
import com.warrenbrowse.vpn.lib.payment.PaymentRepository
import com.warrenbrowse.vpn.lib.payment.ProductIds
import com.warrenbrowse.vpn.lib.payment.model.PaymentAvailability
import com.warrenbrowse.vpn.lib.payment.model.ProductId
import com.warrenbrowse.vpn.lib.payment.model.PurchaseResult
import com.warrenbrowse.vpn.lib.payment.model.VerificationError
import com.warrenbrowse.vpn.lib.payment.model.VerificationResult

class BillingPaymentRepository(
    private val billingRepository: BillingRepository,
    private val playPurchaseRepository: PlayPurchaseRepository,
) : PaymentRepository {

    override fun queryPaymentAvailability(): Flow<PaymentAvailability> = flow {
        emit(PaymentAvailability.Loading)
        val purchases = billingRepository.queryPurchases()
        val productIdToPaymentStatus =
            purchases.purchasesList
                .filter { it.products.isNotEmpty() }
                .associate { it.products.first() to it.purchaseState.toPaymentStatus() }
        emit(
            billingRepository
                .queryProducts(listOf(ProductIds.OneMonth, ProductIds.ThreeMonths))
                .toPaymentAvailability(productIdToPaymentStatus)
        )
    }

    override fun purchaseProduct(
        productId: ProductId,
        activityProvider: () -> Activity,
    ): Flow<PurchaseResult> = flow {
        emit(PurchaseResult.FetchingProducts)

        val productDetailsResult = billingRepository.queryProducts(listOf(productId.value))

        val productDetails =
            when (productDetailsResult.responseCode()) {
                BillingResponseCode.OK -> {
                    productDetailsResult.getProductDetails(productId.value)
                        ?: run {
                            emit(PurchaseResult.Error.NoProductFound(productId))
                            return@flow
                        }
                }
                else -> {
                    emit(
                        PurchaseResult.Error.FetchProductsError(
                            productId,
                            productDetailsResult.toBillingException(),
                        )
                    )
                    return@flow
                }
            }

        // Get transaction id
        emit(PurchaseResult.FetchingObfuscationId)
        val obfuscatedId: PlayExternalObfuscatedAccountId =
            initializePurchase()
                .fold(
                    {
                        emit(PurchaseResult.Error.TransactionIdError(productId, null))
                        return@flow
                    },
                    { it },
                )

        val result =
            billingRepository.startPurchaseFlow(
                productDetails = productDetails,
                obfuscatedId = obfuscatedId,
                activityProvider = activityProvider,
            )

        if (result.responseCode == BillingResponseCode.OK) {
            emit(PurchaseResult.BillingFlowStarted)
        } else {
            emit(
                PurchaseResult.Error.BillingError(
                    BillingException(result.responseCode, result.debugMessage)
                )
            )
            return@flow
        }

        // Wait for a callback from the billing library
        when (val event = billingRepository.purchaseEvents.firstOrNull()) {
            is PurchaseEvent.Error -> emit(event.toPurchaseResult())
            is PurchaseEvent.Completed -> {
                val purchase =
                    event.purchases.firstOrNull()
                        ?: run {
                            emit(PurchaseResult.Error.BillingError(null))
                            return@flow
                        }
                if (purchase.purchaseState == Purchase.PurchaseState.PENDING) {
                    emit(PurchaseResult.Completed.Pending(ProductId(purchase.products.first())))
                } else {
                    emit(PurchaseResult.VerificationStarted)
                    emit(
                        verifyPurchase(purchase)
                            .fold(
                                { error -> error.toPurchaseResultError() },
                                { productId -> PurchaseResult.Completed.Success(productId) },
                            )
                    )
                }
            }
            PurchaseEvent.UserCanceled -> emit(event.toPurchaseResult())
            else -> emit(PurchaseResult.Error.BillingError(null))
        }
    }

    override suspend fun verifyPurchases(): Either<VerificationError, VerificationResult> = either {
        val purchasesResult = billingRepository.queryPurchases()
        ensure(purchasesResult.responseCode() == BillingResponseCode.OK) {
            VerificationError.BillingError(purchasesResult.toBillingException())
        }
        val purchases = purchasesResult.nonPendingPurchases()
        if (purchases.isEmpty()) {
            Logger.d("No purchases to verify")
            return@either VerificationResult.NothingToVerify
        }
        verifyPurchase(purchases.first())
            .mapLeft { it.toPurchaseVerificationError() }
            .map { VerificationResult.Success }
            .bind()
    }

    private suspend fun initializePurchase() =
        playPurchaseRepository.initializePlayPurchase().flatMap {
            if (it.value.isNotEmpty()) {
                it.right()
            } else {
                Logger.e("Obfuscated account id is empty")
                PlayPurchaseInitError.OtherError.left()
            }
        }

    private suspend fun verifyPurchase(
        purchase: Purchase
    ): Either<PlayPurchaseVerifyError, ProductId> =
        either {
                ensure(purchase.products.isNotEmpty()) {
                    Logger.e("Purchase has no products")
                    PlayPurchaseVerifyError.NoProducts
                }
                ensure(purchase.accountIdentifiers?.obfuscatedAccountId != null) {
                    Logger.e("Purchase is missing obfuscatedAccountId")
                    PlayPurchaseVerifyError.MissingObfuscatedAccountId
                }
                ensure(purchase.purchaseToken.isNotEmpty()) {
                    Logger.e("Purchase has no purchase token")
                    PlayPurchaseVerifyError.NoPurchaseToken
                }
                playPurchaseRepository
                    .verifyPlayPurchase(
                        PlayPurchase(
                            productId = purchase.products.first(),
                            purchaseToken = PlayPurchasePaymentToken(purchase.purchaseToken),
                        )
                    )
                    .also { Logger.i("Purchase verification result $it") }
                    .bind()
            }
            .onLeft {
                Logger.e(
                    "Failed to verify purchase token ending with ${purchase.purchaseToken.takeLast(2)}"
                )
            }
            .map { ProductId(purchase.products.first()) }
}
