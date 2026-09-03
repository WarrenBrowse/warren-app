package com.warrenbrowse.vpn.di

import android.content.ComponentName
import android.content.pm.PackageManager
import android.os.Build
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.MainScope
import com.warrenbrowse.vpn.BuildConfig
import com.warrenbrowse.vpn.app.MainActivity
import com.warrenbrowse.vpn.app.product.PROD_APPLICATION_ID
import com.warrenbrowse.vpn.app.product.isApplicationInstalled
import com.warrenbrowse.vpn.app.WarrenAppViewModel
import com.warrenbrowse.vpn.feature.appinfo.impl.AppInfoViewModel
import com.warrenbrowse.vpn.feature.appinfo.impl.changelog.ChangelogViewModel
import com.warrenbrowse.vpn.feature.applisting.api.ResolveAppListingUseCase
import com.warrenbrowse.vpn.feature.applisting.impl.AndroidInstallSourceProvider
import com.warrenbrowse.vpn.feature.applisting.impl.InstallSourceProvider
import com.warrenbrowse.vpn.feature.applisting.impl.ResolveAppListingUseCaseImpl
import com.warrenbrowse.vpn.feature.autoconnect.impl.AutoConnectAndLockdownModeViewModel
import com.warrenbrowse.vpn.feature.home.impl.connect.ConnectViewModel
import com.warrenbrowse.vpn.feature.home.impl.connect.notificationbanner.InAppNotificationController
import com.warrenbrowse.vpn.feature.home.impl.devicerevoked.DeviceRevokedViewModel
import com.warrenbrowse.vpn.feature.language.impl.LanguageViewModel
import com.warrenbrowse.vpn.feature.login.impl.WarrenWalletBackupViewModel
import com.warrenbrowse.vpn.feature.login.impl.WarrenWalletViewModel
import com.warrenbrowse.vpn.feature.notification.impl.NotificationSettingsViewModel
import com.warrenbrowse.vpn.feature.settings.impl.SettingsViewModel
import com.warrenbrowse.vpn.feature.settings.impl.support.ForumActivityViewModel
import com.warrenbrowse.vpn.feature.settings.impl.support.ReportProblemViewModel
import com.warrenbrowse.vpn.feature.splittunneling.impl.SplitTunnelingViewModel
import com.warrenbrowse.vpn.feature.splittunneling.impl.applist.ApplicationsProvider
import com.warrenbrowse.vpn.feature.splittunneling.impl.applist.SplitTunnelingUseCase
import com.warrenbrowse.vpn.feature.splittunneling.impl.search.SearchSplitTunnelingViewModel
import com.warrenbrowse.vpn.lib.model.PackageName
import com.warrenbrowse.vpn.lib.repository.AppVersionInfoRepository
import com.warrenbrowse.vpn.lib.repository.AutoStartAndConnectOnBootRepository
import com.warrenbrowse.vpn.lib.repository.ChangelogDataProvider
import com.warrenbrowse.vpn.lib.repository.ChangelogRepository
import com.warrenbrowse.vpn.lib.repository.RelayListRepository
import com.warrenbrowse.vpn.lib.repository.SplashCompleteRepository
import com.warrenbrowse.vpn.lib.repository.SplitTunnelingRepository
import com.warrenbrowse.vpn.lib.repository.WarrenConnectedInfo
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository
import com.warrenbrowse.vpn.lib.repository.WarrenQuinnDisconnectInvoker
import com.warrenbrowse.vpn.lib.repository.WarrenTunnelStateProvider
import com.warrenbrowse.vpn.lib.usecase.LastKnownLocationUseCase
import com.warrenbrowse.vpn.lib.usecase.SelectedLocationTitleUseCase
import com.warrenbrowse.vpn.lib.usecase.SystemVpnSettingsAvailableUseCase
import com.warrenbrowse.vpn.lib.usecase.inappnotification.AccountExpiryNotificationUseCase
import com.warrenbrowse.vpn.lib.usecase.inappnotification.Android16UpdateWarningUseCase
import com.warrenbrowse.vpn.lib.usecase.inappnotification.ConnectingStuckNotificationUseCase
import com.warrenbrowse.vpn.lib.usecase.inappnotification.ExitEgressNotificationUseCase
import com.warrenbrowse.vpn.lib.usecase.inappnotification.EnvStandDownUseCase
import com.warrenbrowse.vpn.lib.usecase.inappnotification.ExitSwitchedNotificationUseCase
import com.warrenbrowse.vpn.lib.usecase.inappnotification.HostOfflineNotificationUseCase
import com.warrenbrowse.vpn.lib.usecase.inappnotification.InAppNotificationUseCase
import com.warrenbrowse.vpn.lib.usecase.inappnotification.NewChangelogNotificationUseCase
import com.warrenbrowse.vpn.lib.usecase.inappnotification.OperatorNoticeNotificationUseCase
import com.warrenbrowse.vpn.lib.usecase.inappnotification.StandDownSetting
import com.warrenbrowse.vpn.lib.usecase.inappnotification.TunnelStateNotificationUseCase
import com.warrenbrowse.vpn.lib.usecase.inappnotification.UpdateAvailableNotificationUseCase
import com.warrenbrowse.vpn.lib.usecase.inappnotification.VersionNotificationUseCase
import com.warrenbrowse.vpn.receiver.AutoStartVpnBootCompletedReceiver
import com.warrenbrowse.vpn.screen.outoftime.OutOfTimeGateViewModel
import com.warrenbrowse.vpn.screen.privacy.PrivacyDisclaimerViewModel
import com.warrenbrowse.vpn.screen.splash.SplashViewModel
import com.warrenbrowse.vpn.serviceconnection.ServiceConnectionManager
import org.apache.commons.validator.routines.InetAddressValidator
import org.koin.android.ext.koin.androidContext
import org.koin.core.module.dsl.viewModel
import org.koin.core.qualifier.named
import org.koin.dsl.bind
import org.koin.dsl.module

val uiModule = module {
    single<ComponentName>(named(BOOT_COMPLETED_RECEIVER_COMPONENT_NAME)) {
        ComponentName(androidContext(), AutoStartVpnBootCompletedReceiver::class.java)
    }

    single<PackageName> { PackageName(androidContext().packageName) }
    single<InstallSourceProvider> { AndroidInstallSourceProvider(androidContext()) }
    single<ResolveAppListingUseCase> {
        ResolveAppListingUseCaseImpl(
            resources = androidContext().resources,
            packageName = get(),
            isPlayBuild = IS_PLAY_BUILD,
            installSourceProvider = get(),
        )
    }
    single { ApplicationsProvider(get(), get()) }
    scope<MainActivity> { scoped { ServiceConnectionManager(androidContext()) } }
    single { InetAddressValidator.getInstance() }
    single { androidContext().assets }
    single { androidContext().contentResolver }

    single { ChangelogRepository(get(), get(), get()) }
    single { RelayListRepository() }
    single { SplitTunnelingRepository(get()) }
    single { SplitTunnelingUseCase(get(), get(), get(), Dispatchers.IO) }
    single { SplashCompleteRepository() }
    single {
        AutoStartAndConnectOnBootRepository(
            get(),
            get(named(BOOT_COMPLETED_RECEIVER_COMPONENT_NAME)),
        )
    }

    single { TunnelStateNotificationUseCase(get()) } bind
        InAppNotificationUseCase::class
    single { HostOfflineNotificationUseCase(get()) } bind
        InAppNotificationUseCase::class
    // The connect card ORs the wedge verdict into its offline flag; only the
    // banner keeps the two causes apart.
    single { ExitEgressNotificationUseCase(get(), get()) } bind
        InAppNotificationUseCase::class
    single { ConnectingStuckNotificationUseCase(get()) } bind
        InAppNotificationUseCase::class
    // Registered under its own type too: the connect screen acknowledges the
    // switch through it when the banner is dismissed.
    single { ExitSwitchedNotificationUseCase(get()) } bind InAppNotificationUseCase::class
    // Expiry competes for the single banner slot instead of drawing its own
    // strip above it.
    single { AccountExpiryNotificationUseCase(get()) } bind
        InAppNotificationUseCase::class
    single {
        VersionNotificationUseCase(get(), BuildConfig.ENABLE_IN_APP_VERSION_NOTIFICATIONS)
    } bind InAppNotificationUseCase::class
    single {
        val installSourceProvider = get<InstallSourceProvider>()
        UpdateAvailableNotificationUseCase(
            appVersionInfoRepository = get(),
            userPreferencesRepository = get(),
            isVersionInfoNotificationEnabled = BuildConfig.ENABLE_IN_APP_VERSION_NOTIFICATIONS,
            isInstalledFromStore = installSourceProvider::isInstalledFromStore,
        )
    } bind InAppNotificationUseCase::class
    // Ranked first of all: when the operator has published a notice, that
    // message is the one thing the user must read, and the states it hides are
    // still legible in the connect card's own status.
    single { OperatorNoticeNotificationUseCase(get()) } bind InAppNotificationUseCase::class
    single { NewChangelogNotificationUseCase(get()) } bind InAppNotificationUseCase::class
    // Coexistence with a higher-priority product environment (prod over
    // staging over beta). Presence of the other install is the whole rule,
    // because neither mobile OS lets one app read another app's VPN state;
    // the production build looks for nothing, since no environment outranks
    // it. Registered under its own type too: the connect screen brings this
    // build back through it.
    single {
        val localSettings = get<WarrenLocalSettingsRepository>()
        val autoStart = get<AutoStartAndConnectOnBootRepository>()
        val packageManager = get<PackageManager>()
        val tunnelState = get<WarrenTunnelStateProvider>()
        val disconnect = get<WarrenQuinnDisconnectInvoker>()
        EnvStandDownUseCase(
            store = localSettings,
            higherEnvironmentInstalled = {
                BuildConfig.APPLICATION_ID != PROD_APPLICATION_ID &&
                    packageManager.isApplicationInstalled(PROD_APPLICATION_ID)
            },
            // A disconnect dispatch starts the tunnel service when nothing is
            // running, so the teardown only speaks when there is a tunnel to
            // take down.
            stopTunnel = {
                if (tunnelState.connectedInfo.value !is WarrenConnectedInfo.Disconnected) {
                    disconnect.disconnect()
                }
            },
            autoConnect =
                StandDownSetting(
                    read = { autoStart.autoStartAndConnectOnBoot.value },
                    write = autoStart::setAutoStartAndConnectOnBoot,
                ),
            blockingPolicy =
                StandDownSetting(
                    read = { localSettings.lockdownMode.value },
                    write = localSettings::setLockdownMode,
                ),
        )
    } bind InAppNotificationUseCase::class
    if (Build.VERSION.SDK_INT == Build.VERSION_CODES.BAKLAVA) {
        single { Android16UpdateWarningUseCase(get(), get()) } bind InAppNotificationUseCase::class
    }

    single { SystemVpnSettingsAvailableUseCase(androidContext()) }
    // SelectedLocationTitleUseCase + LastKnownLocationUseCase are referenced by
    // ConnectViewModel.
    single { SelectedLocationTitleUseCase(get()) }
    single { LastKnownLocationUseCase(get()) }

    single { InAppNotificationController(getAll(), MainScope()) }

    single { ChangelogDataProvider(get()) }

    single { AppVersionInfoRepository(get(), get()) }

    // View models
    viewModel { params -> ChangelogViewModel(navArgs = params.get(), get(), get()) }
    viewModel {
        AppInfoViewModel(
            appVersionInfoRepository = get(),
            isPlayBuild = IS_PLAY_BUILD,
            resolveAppListing = get(),
        )
    }
    viewModel {
        ConnectViewModel(
            deviceRepository = get(),
            changelogRepository = get(),
            inAppNotificationController = get(),
            userPreferencesRepository = get(),
            selectedLocationTitleUseCase = get(),
            connectionProxy = get(),
            lastKnownLocationUseCase = get(),
            systemVpnSettingsUseCase = get(),
            warrenDisconnect = get(),
            isPlayBuild = IS_PLAY_BUILD,
            resolveAppListing = get(),
            relayProvider = get(),
            pathHealthProvider = get(),
            localSettings = get(),
            hostOfflineProvider = get(),
            autoRecoveryProvider = get(),
            exitSwitchedNotificationUseCase = get(),
            envStandDownUseCase = get(),
        )
    }
    viewModel { DeviceRevokedViewModel(get(), get()) }
    viewModel { WarrenWalletViewModel(get()) }
    // NavBackStackEntry-scoped: consumes MnemonicCache once at init,
    // holds the Mnemonic for the entry's lifetime, zeros it in
    // onCleared. Survives config changes; dies on process kill.
    viewModel { WarrenWalletBackupViewModel() }
    viewModel { PrivacyDisclaimerViewModel(get(), IS_PLAY_BUILD) }
    viewModel { SettingsViewModel(get(), get(), get(), IS_PLAY_BUILD) }
    viewModel { ReportProblemViewModel(get()) }
    viewModel { ForumActivityViewModel(get(), get()) }
    viewModel { SplashViewModel(get(), get(), inject(), inject()) }
    viewModel { OutOfTimeGateViewModel(get(), get()) }
    viewModel { NotificationSettingsViewModel(get(), get(), get()) }
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
        viewModel { LanguageViewModel(get()) }
    }
    viewModel { AutoConnectAndLockdownModeViewModel(isPlayBuild = IS_PLAY_BUILD) }
    viewModel { params ->
        SplitTunnelingViewModel(isModal = params.get(), get(), get(), get(), Dispatchers.IO)
    }

    viewModel { SearchSplitTunnelingViewModel(get(), get(), Dispatchers.IO) }

    // This view model must be single so we correctly attach lifecycle and share it with activity
    single { WarrenAppViewModel() }
}

const val APP_PREFERENCES_NAME = "${BuildConfig.APPLICATION_ID}.app_preferences"
const val KERMIT_FILE_LOG_DIR_NAME = "android_app_logs"

private const val BOOT_COMPLETED_RECEIVER_COMPONENT_NAME = "BOOT_COMPLETED_RECEIVER_COMPONENT_NAME"
// Warren ships a single build with no Play Store in-app purchase billing, so
// this is always false.
private const val IS_PLAY_BUILD = false
