package com.warrenbrowse.vpn.di

import com.warrenbrowse.vpn.app.service.migration.MigrateSplitTunneling
import com.warrenbrowse.vpn.lib.common.constant.CACHE_DIR_NAMED_ARGUMENT
import com.warrenbrowse.vpn.lib.common.constant.FILES_DIR_NAMED_ARGUMENT
import org.koin.android.ext.koin.androidContext
import org.koin.core.qualifier.named
import org.koin.dsl.module

val vpnServiceModule = module {
    single(named(FILES_DIR_NAMED_ARGUMENT)) { androidContext().filesDir }
    single(named(CACHE_DIR_NAMED_ARGUMENT)) { androidContext().cacheDir }

    single { MigrateSplitTunneling(androidContext()) }
}
