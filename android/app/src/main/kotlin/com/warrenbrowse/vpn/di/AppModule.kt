package com.warrenbrowse.vpn.di

import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import androidx.core.app.NotificationManagerCompat
import androidx.datastore.core.DataStore
import androidx.datastore.dataStore
import java.io.File
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.MainScope
import com.warrenbrowse.vpn.BuildConfig
import com.warrenbrowse.vpn.app.connect.WarrenConnectUseCase
import com.warrenbrowse.vpn.app.connect.WarrenTunnelConfigBuilder
import com.warrenbrowse.vpn.feature.appicon.impl.obfuscation.AppObfuscationRepository
import com.warrenbrowse.vpn.feature.settings.impl.WarrenQuinnConnectInvoker
import com.warrenbrowse.vpn.feature.language.impl.LanguageRepository
import com.warrenbrowse.vpn.lib.common.constant.GRPC_SOCKET_FILE_NAME
import com.warrenbrowse.vpn.lib.common.constant.GRPC_SOCKET_FILE_NAMED_ARGUMENT
import com.warrenbrowse.vpn.lib.endpoint.ApiEndpointFromIntentHolder
import com.warrenbrowse.vpn.lib.endpoint.ApiEndpointOverride
import com.warrenbrowse.vpn.lib.grpc.ManagementService
import com.warrenbrowse.vpn.lib.model.BuildVersion
import com.warrenbrowse.vpn.lib.model.NotificationChannel
import com.warrenbrowse.vpn.lib.pushnotification.NotificationChannelFactory
import com.warrenbrowse.vpn.lib.pushnotification.NotificationManager
import com.warrenbrowse.vpn.lib.pushnotification.NotificationProvider
import com.warrenbrowse.vpn.lib.pushnotification.ScheduleNotificationAlarmUseCase
import com.warrenbrowse.vpn.lib.pushnotification.accountexpiry.AccountExpiryNotificationProvider
import com.warrenbrowse.vpn.lib.pushnotification.tunnelstate.TunnelStateNotificationProvider
import com.warrenbrowse.vpn.lib.repository.AccountRepository
import com.warrenbrowse.vpn.lib.repository.AndroidKeystoreWalletRepository
import com.warrenbrowse.vpn.lib.repository.ConnectionProxy
import com.warrenbrowse.vpn.lib.repository.DeviceRepository
import com.warrenbrowse.vpn.lib.repository.LocaleRepository
import com.warrenbrowse.vpn.lib.repository.RelayLocationTranslationRepository
import com.warrenbrowse.vpn.lib.repository.UserPreferencesMigration
import com.warrenbrowse.vpn.lib.repository.UserPreferencesRepository
import com.warrenbrowse.vpn.lib.repository.UserPreferencesSerializer
import com.warrenbrowse.vpn.lib.repository.WalletRepository
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository
import com.warrenbrowse.vpn.lib.usecase.AccountExpiryNotificationActionUseCase
import com.warrenbrowse.vpn.repository.UserPreferences
import org.koin.android.ext.koin.androidContext
import org.koin.core.module.dsl.createdAtStart
import org.koin.core.module.dsl.withOptions
import org.koin.core.qualifier.named
import org.koin.dsl.bind
import org.koin.dsl.module

val appModule = module {
    single(named(GRPC_SOCKET_FILE_NAMED_ARGUMENT)) {
        File(androidContext().noBackupFilesDir, GRPC_SOCKET_FILE_NAME)
    }
    single {
        ManagementService(
            rpcSocketFile = get(named(GRPC_SOCKET_FILE_NAMED_ARGUMENT)),
            extensiveLogging = BuildConfig.DEBUG,
            scope = MainScope(),
        )
    }
    single { ApplicationScope.createDoNotCallUseDiInstead() }

    single { androidContext().resources }
    single { androidContext().userPreferencesStore }
    single { BuildVersion(BuildConfig.VERSION_NAME, BuildConfig.VERSION_CODE) }
    single { ApiEndpointFromIntentHolder() }
    single { AccountRepository(get(), get(), MainScope()) }
    single { DeviceRepository(get()) }
    single { UserPreferencesRepository(get(), get()) }
    single { ConnectionProxy(androidContext(), get(), get()) }
    single<WalletRepository> { AndroidKeystoreWalletRepository(androidContext()) }

    // D.4 step 8: Warren-side tunnel toggles (DAITA / NAT-PMP / multi-hop / M4.0).
    // Kept separate from the proto-backed UserPreferencesRepository so we can
    // drop the legacy Mullvad surface without touching these.
    single { WarrenLocalSettingsRepository(androidContext()) }

    // D.4 step 7 follow-up: orchestrate biometric unlock + config build +
    // service dispatch for Warren Quinn connect.
    single { WarrenTunnelConfigBuilder(localSettings = get()) }
    single {
        WarrenConnectUseCase(walletRepository = get(), configBuilder = get())
    } bind WarrenQuinnConnectInvoker::class
    single { LocaleRepository(get()) }
    single { RelayLocationTranslationRepository(get(), get(), MainScope()) }
    single { ScheduleNotificationAlarmUseCase(androidContext(), get()) }
    single { AccountExpiryNotificationActionUseCase(get(), get()) }
    // TODO Move these back to UiModule when fixDisableBug is removed
    single { AppObfuscationRepository(get(), get()) }
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
        single { LanguageRepository(androidContext()) }
    }
    single<PackageManager> { androidContext().packageManager }

    single { NotificationChannel.TunnelUpdates } bind NotificationChannel::class
    single { NotificationChannel.AccountUpdates } bind NotificationChannel::class
    single { NotificationChannelFactory(get(), get(), getAll()) } withOptions { createdAtStart() }
    single { NotificationManagerCompat.from(androidContext()) }
    single { NotificationManager(get(), getAll(), get(), MainScope()) } withOptions
        {
            createdAtStart()
        }
    single {
        TunnelStateNotificationProvider(
            androidContext(),
            get(),
            get(),
            get(),
            get<NotificationChannel.TunnelUpdates>().id,
            MainScope(),
        )
    } bind NotificationProvider::class
    single { AccountExpiryNotificationProvider(get<NotificationChannel.AccountUpdates>().id) } bind
        NotificationProvider::class
    if (BuildConfig.FLAVOR_infrastructure != "prod") {
        single<ApiEndpointOverride> {
            ApiEndpointOverride(BuildConfig.API_ENDPOINT, BuildConfig.API_IP)
        }
    }
}

private val Context.userPreferencesStore: DataStore<UserPreferences> by
    dataStore(
        fileName = APP_PREFERENCES_NAME,
        serializer = UserPreferencesSerializer,
        produceMigrations = { UserPreferencesMigration.migrations(it, APP_PREFERENCES_NAME) },
    )

class ApplicationScope private constructor(private val cs: CoroutineScope) : CoroutineScope by cs {
    companion object {
        fun createDoNotCallUseDiInstead(): ApplicationScope = ApplicationScope(MainScope())
    }
}
