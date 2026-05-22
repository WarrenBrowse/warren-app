package com.warrenbrowse.vpn.di

import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import androidx.core.app.NotificationManagerCompat
import androidx.datastore.core.DataStore
import androidx.datastore.dataStore
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.MainScope
import com.warrenbrowse.vpn.BuildConfig
import com.warrenbrowse.vpn.app.connect.RelayCatalog
import com.warrenbrowse.vpn.app.connect.WarrenConnectUseCase
import com.warrenbrowse.vpn.app.connect.WarrenDisconnectUseCase
import com.warrenbrowse.vpn.app.connect.WarrenReconnectUseCase
import com.warrenbrowse.vpn.app.connect.WarrenTunnelConfigBuilder
import com.warrenbrowse.vpn.app.service.WarrenQuinnStateProxy
import com.warrenbrowse.vpn.lib.repository.WarrenQuinnConnectInvoker
import com.warrenbrowse.vpn.lib.repository.WarrenQuinnDisconnectInvoker
import com.warrenbrowse.vpn.lib.repository.WarrenQuinnReconnectInvoker
import com.warrenbrowse.vpn.lib.repository.WarrenRelayProvider
import com.warrenbrowse.vpn.lib.repository.WarrenTunnelStateProvider
import com.warrenbrowse.vpn.feature.language.impl.LanguageRepository
import com.warrenbrowse.vpn.lib.endpoint.ApiEndpointFromIntentHolder
import com.warrenbrowse.vpn.lib.endpoint.ApiEndpointOverride
import com.warrenbrowse.vpn.lib.model.BuildVersion
import com.warrenbrowse.vpn.lib.model.NotificationChannel
import com.warrenbrowse.vpn.lib.pushnotification.NotificationChannelFactory
import com.warrenbrowse.vpn.lib.pushnotification.NotificationManager
import com.warrenbrowse.vpn.lib.pushnotification.NotificationProvider
import com.warrenbrowse.vpn.lib.pushnotification.tunnelstate.TunnelStateNotificationProvider
import com.warrenbrowse.vpn.lib.repository.AccountRepository
import com.warrenbrowse.vpn.lib.repository.AndroidKeystoreWalletRepository
import com.warrenbrowse.vpn.lib.repository.ConnectionProxy
import com.warrenbrowse.vpn.lib.repository.DeviceRepository
import com.warrenbrowse.vpn.lib.repository.LocaleRepository
import com.warrenbrowse.vpn.lib.repository.UserPreferencesMigration
import com.warrenbrowse.vpn.lib.repository.UserPreferencesRepository
import com.warrenbrowse.vpn.lib.repository.UserPreferencesSerializer
import com.warrenbrowse.vpn.lib.repository.WalletRepository
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository
import com.warrenbrowse.vpn.repository.UserPreferences
import org.koin.android.ext.koin.androidContext
import org.koin.core.module.dsl.createdAtStart
import org.koin.core.module.dsl.withOptions
import org.koin.dsl.bind
import org.koin.dsl.module

val appModule = module {
    // D.4 step 58: ManagementService koin single dropped (Mullvad daemon gRPC
    // bridge dead on Warren — all repository consumers slimmed to Warren-
    // native stubs).
    single { ApplicationScope.createDoNotCallUseDiInstead() }

    single { androidContext().resources }
    single { androidContext().userPreferencesStore }
    single { BuildVersion(BuildConfig.VERSION_NAME, BuildConfig.VERSION_CODE) }
    single { ApiEndpointFromIntentHolder() }
    // D.4 step 58: AccountRepository slimmed to Warren-native stub.
    single { AccountRepository() }
    // D.4 step 58: DeviceRepository slimmed to Warren-native stub.
    single { DeviceRepository() }
    single { UserPreferencesRepository(get(), get()) }
    // D.4 step 58: ConnectionProxy slimmed to Warren-native stub (no
    // ManagementService / RelayLocationTranslationRepository deps).
    // D.4 step 59: rewired to WarrenTunnelStateProvider (real tunnel state).
    single { ConnectionProxy(tunnelStateProvider = get()) }
    single<WalletRepository> { AndroidKeystoreWalletRepository(androidContext()) }

    // D.4 step 8: Warren-side tunnel toggles (DAITA / NAT-PMP / multi-hop / M4.0).
    // Kept separate from the proto-backed UserPreferencesRepository so we can
    // drop the legacy Mullvad surface without touching these.
    single { WarrenLocalSettingsRepository(androidContext()) }

    // D.4 step 17: relay catalogue via WarrenJni.listRelays. Hardcoded entry
    // today; D.6 wires the signed-relay-list fetch via warren-api-client.
    single { RelayCatalog() } bind WarrenRelayProvider::class

    // D.4 step 9: process-singleton mirror of WarrenQuinnAdapter.state so
    // Composables can read tunnel transitions without binding the service.
    single { WarrenQuinnStateProxy() } bind WarrenTunnelStateProvider::class

    // D.4 step 7 follow-up: orchestrate biometric unlock + config build +
    // service dispatch for Warren Quinn connect.
    single { WarrenTunnelConfigBuilder(localSettings = get(), relayCatalog = get()) }
    single {
        WarrenConnectUseCase(walletRepository = get(), configBuilder = get())
    } bind WarrenQuinnConnectInvoker::class

    // D.4 step 12: disconnect path bound to the same lib-side surface
    // contract; Connect button + tile service + notification action all
    // resolve this single binding.
    single { WarrenDisconnectUseCase(context = androidContext()) } bind
        WarrenQuinnDisconnectInvoker::class

    single { WarrenReconnectUseCase(context = androidContext()) } bind
        WarrenQuinnReconnectInvoker::class
    single { LocaleRepository(get()) }
    // D.4 step 58: RelayLocationTranslationRepository dropped (orphan now).
    // D.4 step 38: ScheduleNotificationAlarmUseCase + AccountExpiryNotification-
    // ActionUseCase dropped (Mullvad subscription expiry notifications dead).
    // TODO Move these back to UiModule when fixDisableBug is removed
    // D.4 step 61: AppObfuscationRepository dropped (Mullvad app-icon
    // obfuscation feature dead - Warren is not Mullvad-branded).
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
        single { LanguageRepository(androidContext()) }
    }
    single<PackageManager> { androidContext().packageManager }

    single { NotificationChannel.TunnelUpdates } bind NotificationChannel::class
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
