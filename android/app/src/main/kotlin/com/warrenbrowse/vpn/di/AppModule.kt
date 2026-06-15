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
import com.warrenbrowse.vpn.app.connect.WarrenSendProblemReportUseCase
import com.warrenbrowse.vpn.app.connect.WarrenSubscriptionUseCase
import com.warrenbrowse.vpn.app.connect.WarrenTunnelConfigBuilder
import com.warrenbrowse.vpn.app.service.WarrenQuinnStateProxy
import com.warrenbrowse.vpn.jni.WarrenJniBridgeImpl
import com.warrenbrowse.vpn.lib.repository.WarrenJniBridge
import com.warrenbrowse.vpn.lib.repository.WarrenQuinnConnectInvoker
import com.warrenbrowse.vpn.lib.repository.WarrenQuinnDisconnectInvoker
import com.warrenbrowse.vpn.lib.repository.WarrenQuinnReconnectInvoker
import com.warrenbrowse.vpn.lib.repository.WarrenRelayProvider
import com.warrenbrowse.vpn.lib.repository.WarrenSubscriptionInvoker
import com.warrenbrowse.vpn.lib.repository.WarrenSupportReportInvoker
import com.warrenbrowse.vpn.lib.repository.WarrenNatPmpStatusProvider
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
import org.koin.dsl.binds
import org.koin.dsl.module

val appModule = module {
    single { ApplicationScope.createDoNotCallUseDiInstead() }

    single { androidContext().resources }
    single { androidContext().userPreferencesStore }
    single { BuildVersion(BuildConfig.VERSION_NAME, BuildConfig.VERSION_CODE) }
    single { ApiEndpointFromIntentHolder() }
    single { AccountRepository() }
    single { DeviceRepository() }
    single { UserPreferencesRepository(get(), get()) }
    // ConnectionProxy reads live tunnel state from WarrenTunnelStateProvider.
    single { ConnectionProxy(tunnelStateProvider = get()) }

    // D.6 audit follow-up: lib/repository consumes the JNI surface via
    // this interface (lives in lib/repository). The concrete impl
    // lives in `:app/jni/WarrenJniBridgeImpl` so the `lib/<x>` modules
    // never reach into `:app`.
    single<WarrenJniBridge> { WarrenJniBridgeImpl() }

    single<WalletRepository> {
        AndroidKeystoreWalletRepository(context = androidContext(), jni = get())
    }

    // Warren-side tunnel toggles (DAITA / NAT-PMP / multi-hop / M4.0), kept
    // separate from the proto-backed UserPreferencesRepository.
    single { WarrenLocalSettingsRepository(androidContext()) }

    // Relay catalogue via WarrenJni.listRelays.
    single { RelayCatalog() } bind WarrenRelayProvider::class

    // Process-singleton mirror of WarrenQuinnAdapter.state so Composables can
    // read tunnel transitions without binding the service.
    single { WarrenQuinnStateProxy() } binds
        arrayOf(WarrenTunnelStateProvider::class, WarrenNatPmpStatusProvider::class)

    // Orchestrates biometric unlock + config build + service dispatch for the
    // Warren Quinn connect.
    single { WarrenTunnelConfigBuilder(localSettings = get(), relayCatalog = get()) }
    single {
        WarrenConnectUseCase(
            walletRepository = get(),
            configBuilder = get(),
            localSettings = get(),
        )
    } bind WarrenQuinnConnectInvoker::class

    // Disconnect path bound to the lib-side surface contract; Connect button +
    // tile service + notification action all resolve this single binding.
    single { WarrenDisconnectUseCase(context = androidContext()) } bind
        WarrenQuinnDisconnectInvoker::class

    single { WarrenReconnectUseCase(context = androidContext()) } bind
        WarrenQuinnReconnectInvoker::class

    // D.6 support-report submission orchestrator: biometric unlock + JNI
    // sign + POST /v1/support. Activity-coupled because of the biometric
    // prompt; the lib-side ReportProblemScreen invokes this via the
    // WarrenSupportReportInvoker surface and feeds it the FragmentActivity.
    single { WarrenSendProblemReportUseCase(walletRepository = get()) } bind
        WarrenSupportReportInvoker::class

    // Subscription-status fetch: biometric unlock + signed GET /v1/subscription.
    single { WarrenSubscriptionUseCase(walletRepository = get(), localSettings = get()) } bind
        WarrenSubscriptionInvoker::class

    single { LocaleRepository(get()) }
    // TODO Move these back to UiModule when fixDisableBug is removed
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
    // With a single flavor dimension AGP does not emit `FLAVOR_infrastructure`;
    // the canonical flavor name surfaces via `BuildConfig.FLAVOR` directly.
    if (BuildConfig.FLAVOR != "prod") {
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
