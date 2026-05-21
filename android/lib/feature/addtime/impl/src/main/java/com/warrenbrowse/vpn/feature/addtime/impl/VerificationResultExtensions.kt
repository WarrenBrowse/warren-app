package com.warrenbrowse.vpn.feature.addtime.impl

import arrow.core.Either
import com.warrenbrowse.vpn.lib.payment.model.VerificationError
import com.warrenbrowse.vpn.lib.payment.model.VerificationResult

fun Either<VerificationError, VerificationResult>.isSuccess() =
    getOrNull() == VerificationResult.Success
