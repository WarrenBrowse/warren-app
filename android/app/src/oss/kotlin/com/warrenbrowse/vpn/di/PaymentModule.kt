package com.warrenbrowse.vpn.di

import com.warrenbrowse.vpn.lib.payment.PaymentProvider
import org.koin.dsl.module

val paymentModule = module { single { PaymentProvider(null) } }
