package com.warrenbrowse.vpn.di

import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import androidx.core.app.NotificationManagerCompat
import androidx.datastore.core.DataStore
import androidx.datastore.dataStore
import com.warrenbrowse.vpn.BuildConfig
import com.warrenbrowse.vpn.app.connect.RelayCatalog
import com.warrenbrowse.vpn.app.connect.WarrenConnectUseCase
import com.warrenbrowse.vpn.app.connect.WarrenIncidentReportUseCase
import com.warrenbrowse.vpn.app.connect.WarrenDisconnectUseCase
import com.warrenbrowse.vpn.app.connect.WarrenReconnectUseCase
import com.warrenbrowse.vpn.app.connect.WarrenSubscriptionUseCase
import com.warrenbrowse.vpn.app.connect.WarrenTunnelConfigBuilder
import com.warrenbrowse.vpn.app.connectivity.WarrenConnectivityMonitor
import com.warrenbrowse.vpn.app.forum.ForumDigestPoller
import com.warrenbrowse.vpn.app.announcements.WarrenAnnouncementPoller
import com.warrenbrowse.vpn.app.notices.WarrenNoticePoller
import com.warrenbrowse.vpn.app.forum.ForumEvent
import com.warrenbrowse.vpn.app.forum.ForumEventsJournal
import com.warrenbrowse.vpn.app.forum.ForumJournal
import com.warrenbrowse.vpn.app.forum.ForumLoginController
import com.warrenbrowse.vpn.app.forum.JournalField
import com.warrenbrowse.vpn.app.forum.LinkSource
import com.warrenbrowse.vpn.app.forum.WarrenForumActivityUseCase
import com.warrenbrowse.vpn.app.forum.WarrenForumLoginUseCase
import com.warrenbrowse.vpn.app.forum.WarrenSupportReporterImpl
import com.warrenbrowse.vpn.app.forum.forumLoginLinkFromCode
import com.warrenbrowse.vpn.app.network.WarrenNetworkInfoUseCase
import com.warrenbrowse.vpn.app.service.WarrenQuinnStateProxy
import com.warrenbrowse.vpn.feature.language.impl.LanguageRepository
import com.warrenbrowse.vpn.jni.WarrenJniBridgeImpl
import com.warrenbrowse.vpn.lib.endpoint.ApiEndpointFromIntentHolder
import com.warrenbrowse.vpn.lib.endpoint.ApiEndpointOverride
import com.warrenbrowse.vpn.lib.model.BuildVersion
import com.warrenbrowse.vpn.lib.model.NotificationChannel
import com.warrenbrowse.vpn.lib.pushnotification.NotificationChannelFactory
import com.warrenbrowse.vpn.lib.pushnotification.NotificationManager
import com.warrenbrowse.vpn.lib.pushnotification.NotificationProvider
import com.warrenbrowse.vpn.lib.pushnotification.forum.ForumActivityNotificationProvider
import com.warrenbrowse.vpn.lib.pushnotification.tunnelstate.TunnelStateNotificationProvider
import com.warrenbrowse.vpn.lib.repository.AccountRepository
import com.warrenbrowse.vpn.lib.repository.AndroidKeystoreWalletRepository
import com.warrenbrowse.vpn.lib.repository.ConnectionProxy
import com.warrenbrowse.vpn.lib.repository.DeviceRepository
import com.warrenbrowse.vpn.lib.repository.ForumActivityAlerts
import com.warrenbrowse.vpn.lib.repository.ForumActivityOpenRequests
import com.warrenbrowse.vpn.lib.repository.ForumActivityRepository
import com.warrenbrowse.vpn.lib.repository.ForumActivityState
import com.warrenbrowse.vpn.lib.repository.ForumIdentityRepository
import com.warrenbrowse.vpn.lib.repository.ForumNotificationsReader
import com.warrenbrowse.vpn.lib.repository.ForumIdentityWalletBinding
import com.warrenbrowse.vpn.lib.repository.ForumSignInRequests
import com.warrenbrowse.vpn.lib.repository.LocaleRepository
import com.warrenbrowse.vpn.lib.repository.SharedPreferencesForumIdentityRepository
import com.warrenbrowse.vpn.lib.repository.UserPreferencesMigration
import com.warrenbrowse.vpn.lib.repository.UserPreferencesRepository
import com.warrenbrowse.vpn.lib.repository.UserPreferencesSerializer
import com.warrenbrowse.vpn.lib.repository.WalletRepository
import com.warrenbrowse.vpn.lib.repository.WarrenAutoRecoveryProvider
import com.warrenbrowse.vpn.lib.repository.WarrenFailoverProvider
import com.warrenbrowse.vpn.lib.repository.WarrenHostOfflineProvider
import com.warrenbrowse.vpn.lib.repository.WarrenJniBridge
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository
import com.warrenbrowse.vpn.lib.repository.WarrenNatPmpStatusProvider
import com.warrenbrowse.vpn.lib.repository.WarrenNetworkInfoProvider
import com.warrenbrowse.vpn.lib.repository.WarrenAnnouncementRepository
import com.warrenbrowse.vpn.lib.repository.WarrenAnnouncementState
import com.warrenbrowse.vpn.lib.repository.WarrenNoticeRepository
import com.warrenbrowse.vpn.lib.repository.WarrenNoticeState
import com.warrenbrowse.vpn.lib.repository.WarrenPathHealthProvider
import com.warrenbrowse.vpn.lib.repository.WarrenPathMetricsProvider
import com.warrenbrowse.vpn.lib.repository.WarrenProductFlags
import com.warrenbrowse.vpn.lib.repository.WarrenIncidentReporter
import com.warrenbrowse.vpn.lib.repository.WarrenQuinnConnectInvoker
import com.warrenbrowse.vpn.lib.repository.WarrenQuinnDisconnectInvoker
import com.warrenbrowse.vpn.lib.repository.WarrenQuinnReconnectInvoker
import com.warrenbrowse.vpn.lib.repository.WarrenRelayProvider
import com.warrenbrowse.vpn.lib.repository.WarrenSubscriptionInvoker
import com.warrenbrowse.vpn.lib.repository.WarrenSupportReporter
import com.warrenbrowse.vpn.lib.repository.WarrenTunnelStateProvider
import com.warrenbrowse.vpn.repository.UserPreferences
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.MainScope
import kotlinx.coroutines.SupervisorJob
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

    // lib/repository consumes the JNI surface via
    // this interface (lives in lib/repository). The concrete impl
    // lives in `:app/jni/WarrenJniBridgeImpl` so the `lib/<x>` modules
    // never reach into `:app`.
    single<WarrenJniBridge> { WarrenJniBridgeImpl() }

    single<WalletRepository> {
        AndroidKeystoreWalletRepository(context = androidContext(), jni = get())
    }

    // Warren-side tunnel toggles (DAITA / NAT-PMP / multi-hop), kept
    // separate from the proto-backed UserPreferencesRepository.
    single { WarrenLocalSettingsRepository(androidContext()) }

    // Relay catalogue via WarrenJni.listRelays.
    single { RelayCatalog() } bind WarrenRelayProvider::class

    // Process-singleton mirror of WarrenQuinnAdapter.state so Composables can
    // read tunnel transitions without binding the service.
    single { WarrenQuinnStateProxy() } binds
        arrayOf(
            WarrenTunnelStateProvider::class,
            WarrenNatPmpStatusProvider::class,
            WarrenAutoRecoveryProvider::class,
            WarrenPathHealthProvider::class,
            WarrenFailoverProvider::class,
            WarrenPathMetricsProvider::class,
        )

    // Process-wide truthful connectivity source: feeds the adapter's
    // connect/retry gating and the UI host-offline honesty surfaces.
    // The scope must not be the main one: resolving the status opens and
    // connects UDP sockets to probe each IP family, which blocks the caller
    // (talpid's own ConnectivityListener collects the same flow on IO).
    single {
        WarrenConnectivityMonitor(
            connectivityManager =
                androidContext().getSystemService(Context.CONNECTIVITY_SERVICE)
                    as android.net.ConnectivityManager,
            scope = CoroutineScope(SupervisorJob() + Dispatchers.IO),
        )
    } bind WarrenHostOfflineProvider::class

    // Orchestrates biometric unlock + config build + service dispatch for the
    // Warren Quinn connect.
    single {
        WarrenTunnelConfigBuilder(localSettings = get(), productFlags = get(), relayCatalog = get())
    }
    single {
        WarrenConnectUseCase(
            walletRepository = get(),
            configBuilder = get(),
            localSettings = get(),
            connectionProxy = get(),
        )
    } bind WarrenQuinnConnectInvoker::class

    // Disconnect path bound to the lib-side surface contract; Connect button +
    // tile service + notification action all resolve this single binding.
    single { WarrenDisconnectUseCase(context = androidContext(), connectionProxy = get()) } bind
        WarrenQuinnDisconnectInvoker::class

    single { WarrenReconnectUseCase(context = androidContext(), connectionProxy = get()) } bind
        WarrenQuinnReconnectInvoker::class

    // The user-driven incident report behind the key-mismatch dialog's
    // "Report to Warren"; the automatic exit-down report needs no binding,
    // the tunnel adapter posts it through its own platform seam.
    single { WarrenIncidentReportUseCase(walletRepository = get(), jni = get()) } bind
        WarrenIncidentReporter::class

    // Subscription-status fetch: biometric unlock + signed GET /v1/subscription.
    single { WarrenSubscriptionUseCase(walletRepository = get(), localSettings = get()) } bind
        WarrenSubscriptionInvoker::class

    // Community-forum wallet login (doc 55): the deep-link consent controller and
    // the sign + POST use case. `WarrenJni.forumLogin` signs AND sends in Rust.
    single { ForumLoginController() }
    single<ForumIdentityRepository> { SharedPreferencesForumIdentityRepository(androidContext()) }
    // Erasing the wallet erases the forum name learnt under it. Resolved by
    // the application's IO warm-up rather than at Koin start: both
    // repositories it needs read their preferences at construction, and Koin
    // start runs on the main thread.
    single {
        ForumIdentityWalletBinding(
                wallet = get(),
                forumIdentity = get(),
                scope = get<ApplicationScope>(),
            )
            .also { it.start() }
    }
    single<ForumJournal> {
        ForumEventsJournal(
            logDir = androidContext().filesDir.resolve(KERMIT_FILE_LOG_DIR_NAME),
            scope = get<ApplicationScope>(),
        )
    }
    single {
        WarrenForumLoginUseCase(
            walletRepository = get(),
            forumIdentityRepository = get(),
            journal = get(),
            jni = get(),
            tunnelState = get(),
        )
    }
    // The sign-in code typed by hand lands on the same consent prompt.
    single<ForumSignInRequests> {
        val controller = get<ForumLoginController>()
        val journal = get<ForumJournal>()
        ForumSignInRequests { sid ->
            journal.record(
                ForumEvent.LINK_RECEIVED,
                JournalField.Verdict("accepted"),
                JournalField.Source(LinkSource.TYPED_CODE),
            )
            controller.request(forumLoginLinkFromCode(sid))
        }
    }
    // The forum activity badge (doc 55): one number for the bell, the
    // notification and the panel, from the broadcast digest indexed by the
    // wallet's slot, corrected by what the panel itself proved. The alerts
    // surface is the notification provider registered below.
    single {
        ForumActivityRepository(
                identity = get<ForumIdentityRepository>().identity,
                enabled = get<WarrenLocalSettingsRepository>().forumNotificationsEnabled,
                alerts = get(),
                scope = get<ApplicationScope>(),
            )
            .also { it.start() }
    } bind ForumActivityState::class
    single<ForumNotificationsReader> {
        WarrenForumActivityUseCase(
            walletRepository = get(),
            forumIdentityRepository = get(),
            activity = get(),
            jni = get(),
            tunnelState = get(),
        )
    }
    single { ForumDigestPoller(jni = get(), activity = get(), tunnelState = get()) }
    // The operator broadcast banner (doc 55): one signed message from the
    // operator, verified in Rust and shown above every other banner. The state
    // is in memory only, so an erased notice can never come back off a disk
    // cache.
    single { WarrenNoticeRepository() } bind WarrenNoticeState::class
    single {
        WarrenNoticePoller(
            jni = get(),
            state = get(),
            tunnelState = get(),
            clientVersion = BuildConfig.VERSION_NAME,
        )
    }
    // The launch announcement card: a signed broadcast document, plus the
    // wallet-signed lookup of the code this account was pre-assigned. The state
    // is in memory only, so a withdrawn card cannot come back off a disk cache
    // and no code outlives the campaign on disk; only the reader's dismissal is
    // persisted, in the user preferences.
    single { WarrenAnnouncementRepository() } bind WarrenAnnouncementState::class
    single {
        WarrenAnnouncementPoller(
            jni = get(),
            state = get(),
            tunnelState = get(),
            wallet = get(),
            clientVersion = BuildConfig.VERSION_NAME,
        )
    }
    single { ForumActivityOpenRequests() }
    single<WarrenSupportReporter> {
        WarrenSupportReporterImpl(
            context = androidContext(),
            jni = get(),
            walletRepository = get(),
            forumIdentityRepository = get(),
            tunnelState = get(),
            journal = get(),
            appLogDir = androidContext().filesDir.resolve(KERMIT_FILE_LOG_DIR_NAME),
        )
    }

    single { LocaleRepository(get()) }
    // TODO Move these back to UiModule when fixDisableBug is removed
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
        single { LanguageRepository(androidContext()) }
    }
    single<PackageManager> { androidContext().packageManager }

    single { NotificationChannel.TunnelUpdates } bind NotificationChannel::class
    single { NotificationChannel.ForumActivity } bind NotificationChannel::class
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
    single { ForumActivityNotificationProvider(get<NotificationChannel.ForumActivity>().id) } binds
        arrayOf(NotificationProvider::class, ForumActivityAlerts::class)
    // Compile-time product facts for lib modules (they cannot read the
    // app BuildConfig): the beta flavor drives the BETA banner and the
    // payment-surface masking.
    single { WarrenProductFlags(isBeta = BuildConfig.FLAVOR == "beta") }

    // Public /v1/network descriptor feed (display data: the beta banner
    // reads the live bandwidth cap from it).
    single { WarrenNetworkInfoUseCase(bridge = get(), scope = MainScope()) } bind
        WarrenNetworkInfoProvider::class

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
