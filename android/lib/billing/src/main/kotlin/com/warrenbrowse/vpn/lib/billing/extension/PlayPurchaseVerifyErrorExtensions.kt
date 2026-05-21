package com.warrenbrowse.vpn.lib.billing.extension

import com.warrenbrowse.vpn.lib.model.PlayPurchaseVerifyError
import com.warrenbrowse.vpn.lib.payment.model.PurchaseResult
import com.warrenbrowse.vpn.lib.payment.model.VerificationError

fun PlayPurchaseVerifyError.toPurchaseVerificationError(): VerificationError.PlayVerificationError =
    when (this) {
        PlayPurchaseVerifyError.NoProducts,
        PlayPurchaseVerifyError.MissingObfuscatedAccountId,
        PlayPurchaseVerifyError.NoPurchaseToken,
        PlayPurchaseVerifyError.InvalidPurchase ->
            VerificationError.PlayVerificationError.VerificationFailed
        PlayPurchaseVerifyError.OtherError -> VerificationError.PlayVerificationError.Other
    }

fun PlayPurchaseVerifyError.toPurchaseResultError(): PurchaseResult.Error.VerificationError =
    when (this) {
        PlayPurchaseVerifyError.NoProducts,
        PlayPurchaseVerifyError.MissingObfuscatedAccountId,
        PlayPurchaseVerifyError.NoPurchaseToken,
        PlayPurchaseVerifyError.InvalidPurchase ->
            PurchaseResult.Error.VerificationError.VerificationFailed
        PlayPurchaseVerifyError.OtherError -> PurchaseResult.Error.VerificationError.Other
    }
