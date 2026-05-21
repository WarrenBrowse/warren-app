package com.warrenbrowse.vpn.di

import com.warrenbrowse.vpn.lib.billing.BillingPaymentRepository
import com.warrenbrowse.vpn.lib.billing.BillingRepository
import com.warrenbrowse.vpn.lib.billing.PlayPurchaseRepository
import com.warrenbrowse.vpn.lib.payment.PaymentProvider
import org.koin.android.ext.koin.androidContext
import org.koin.dsl.module

val paymentModule = module {
    single { BillingRepository(androidContext()) }
    single { PaymentProvider(BillingPaymentRepository(get(), get())) }
    single { PlayPurchaseRepository(get()) }
}
