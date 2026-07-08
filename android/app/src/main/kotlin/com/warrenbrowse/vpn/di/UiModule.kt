package com.warrenbrowse.vpn.di

import android.content.ComponentName
import android.os.Build
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.MainScope
import com.warrenbrowse.vpn.BuildConfig
import com.warrenbrowse.vpn.app.MainActivity
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
import com.warrenbrowse.vpn.feature.problemreport.impl.ReportProblemViewModel
import com.warrenbrowse.vpn.feature.problemreport.impl.viewlogs.ViewLogsViewModel
import com.warrenbrowse.vpn.feature.settings.impl.SettingsViewModel
import com.warrenbrowse.vpn.feature.splittunneling.impl.SplitTunnelingViewModel
import com.warrenbrowse.vpn.feature.splittunneling.impl.applist.ApplicationsProvider
import com.warrenbrowse.vpn.feature.splittunneling.impl.applist.SplitTunnelingUseCase
import com.warrenbrowse.vpn.feature.splittunneling.impl.search.SearchSplitTunnelingViewModel
import com.warrenbrowse.vpn.lib.model.PackageName
import com.warrenbrowse.vpn.lib.repository.AppVersionInfoRepository
import com.warrenbrowse.vpn.lib.repository.AutoStartAndConnectOnBootRepository
import com.warrenbrowse.vpn.lib.repository.ChangelogDataProvider
import com.warrenbrowse.vpn.lib.repository.ChangelogRepository
import com.warrenbrowse.vpn.lib.repository.ProblemReportRepository
import com.warrenbrowse.vpn.lib.repository.RelayListRepository
import com.warrenbrowse.vpn.lib.repository.SplashCompleteRepository
import com.warrenbrowse.vpn.lib.repository.SplitTunnelingRepository
import com.warrenbrowse.vpn.lib.usecase.LastKnownLocationUseCase
import com.warrenbrowse.vpn.lib.usecase.SelectedLocationTitleUseCase
import com.warrenbrowse.vpn.lib.usecase.SystemVpnSettingsAvailableUseCase
import com.warrenbrowse.vpn.lib.usecase.inappnotification.Android16UpdateWarningUseCase
import com.warrenbrowse.vpn.lib.usecase.inappnotification.HostOfflineNotificationUseCase
import com.warrenbrowse.vpn.lib.usecase.inappnotification.InAppNotificationUseCase
import com.warrenbrowse.vpn.lib.usecase.inappnotification.NewChangelogNotificationUseCase
import com.warrenbrowse.vpn.lib.usecase.inappnotification.TunnelStateNotificationUseCase
import com.warrenbrowse.vpn.lib.usecase.inappnotification.UpdateAvailableNotificationUseCase
import com.warrenbrowse.vpn.lib.usecase.inappnotification.VersionNotificationUseCase
import com.warrenbrowse.vpn.receiver.AutoStartVpnBootCompletedReceiver
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
    single { ProblemReportRepository(context = androidContext(), jni = get()) }
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
    single {
        VersionNotificationUseCase(get(), BuildConfig.ENABLE_IN_APP_VERSION_NOTIFICATIONS)
    } bind InAppNotificationUseCase::class
    single {
        val installSourceProvider = get<InstallSourceProvider>()
        UpdateAvailableNotificationUseCase(
            appVersionInfoRepository = get(),
            isVersionInfoNotificationEnabled = BuildConfig.ENABLE_IN_APP_VERSION_NOTIFICATIONS,
            isInstalledFromStore = installSourceProvider::isInstalledFromStore,
        )
    } bind InAppNotificationUseCase::class
    single { NewChangelogNotificationUseCase(get()) } bind InAppNotificationUseCase::class
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
            warrenReconnect = get(),
            isPlayBuild = IS_PLAY_BUILD,
            resolveAppListing = get(),
            relayProvider = get(),
            localSettings = get(),
            hostOfflineProvider = get(),
            autoRecoveryProvider = get(),
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
    viewModel { SplashViewModel(get(), get(), get(), get()) }
    viewModel {
        ReportProblemViewModel(
            problemReportRepository = get(),
            isPlayBuild = IS_PLAY_BUILD,
            supportReportInvoker = get(),
        )
    }
    viewModel { ViewLogsViewModel(get()) }
    viewModel { NotificationSettingsViewModel(get()) }
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
