package com.warrenbrowse.vpn.di

import android.content.ComponentName
import android.os.Build
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.MainScope
import com.warrenbrowse.vpn.BuildConfig
import com.warrenbrowse.vpn.app.MainActivity
import com.warrenbrowse.vpn.app.WarrenAppViewModel
import com.warrenbrowse.vpn.feature.appicon.impl.AppIconViewModel
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
import com.warrenbrowse.vpn.feature.login.impl.WarrenWalletViewModel
import com.warrenbrowse.vpn.feature.notification.impl.NotificationSettingsViewModel
import com.warrenbrowse.vpn.feature.problemreport.impl.ReportProblemViewModel
import com.warrenbrowse.vpn.feature.problemreport.impl.viewlogs.ViewLogsViewModel
import com.warrenbrowse.vpn.feature.settings.impl.SettingsViewModel
import com.warrenbrowse.vpn.feature.splittunneling.impl.SplitTunnelingViewModel
import com.warrenbrowse.vpn.feature.splittunneling.impl.applist.ApplicationsProvider
import com.warrenbrowse.vpn.feature.splittunneling.impl.applist.SplitTunnelingUseCase
import com.warrenbrowse.vpn.feature.splittunneling.impl.search.SearchSplitTunnelingViewModel
import com.warrenbrowse.vpn.lib.common.constant.BillingTypes
import com.warrenbrowse.vpn.lib.model.PackageName
import com.warrenbrowse.vpn.lib.repository.AppVersionInfoRepository
import com.warrenbrowse.vpn.lib.repository.AutoStartAndConnectOnBootRepository
import com.warrenbrowse.vpn.lib.repository.ChangelogDataProvider
import com.warrenbrowse.vpn.lib.repository.ChangelogRepository
import com.warrenbrowse.vpn.lib.repository.ProblemReportRepository
import com.warrenbrowse.vpn.lib.repository.RelayListRepository
import com.warrenbrowse.vpn.lib.repository.SettingsRepository
import com.warrenbrowse.vpn.lib.repository.SplashCompleteRepository
import com.warrenbrowse.vpn.lib.repository.SplitTunnelingRepository
import com.warrenbrowse.vpn.lib.repository.WireguardConstraintsRepository
import com.warrenbrowse.vpn.lib.usecase.DeleteCustomDnsUseCase
import com.warrenbrowse.vpn.lib.usecase.LastKnownLocationUseCase
import com.warrenbrowse.vpn.lib.usecase.SelectedLocationTitleUseCase
import com.warrenbrowse.vpn.lib.usecase.SystemVpnSettingsAvailableUseCase
import com.warrenbrowse.vpn.lib.usecase.inappnotification.Android16UpdateWarningUseCase
import com.warrenbrowse.vpn.lib.usecase.inappnotification.InAppNotificationUseCase
import com.warrenbrowse.vpn.lib.usecase.inappnotification.NewChangelogNotificationUseCase
import com.warrenbrowse.vpn.lib.usecase.inappnotification.TunnelStateNotificationUseCase
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
    single { SettingsRepository(get()) }
    single {
        ProblemReportRepository(
            context = androidContext(),
            apiEndpointOverride = getOrNull(),
            apiEndpointFromIntentHolder = get(),
            kermitFileLogDirName = KERMIT_FILE_LOG_DIR_NAME,
            accountRepository = get(),
        )
    }
    // D.4 step 35: RelayOverridesRepository removed - Warren exit fleet is
    // sovereign, no per-relay IP overrides.
    // D.4 step 45: CustomListsRepository + RelayListFilterRepository dropped
    // (orphan singles — CustomList/Filter screens deleted in step 27,
    // SelectedLocationTitleUseCase rewritten without CustomLists dependency).
    single { RelayListRepository(get(), get()) }
    // D.4 step 29: VoucherRepository removed - only consumer was the
    // deleted VoucherDialogViewModel.
    single { SplitTunnelingRepository(get()) }
    single { SplitTunnelingUseCase(get(), get(), get(), Dispatchers.IO) }
    // D.4 step 33: ApiAccessRepository removed - Mullvad-only API
    // access method configuration (HTTPS proxies/Tor bridges to reach
    // mullvad.net). Warren's API endpoint is fixed at build time.
    // D.4 step 41: NewDeviceRepository dropped (Mullvad multi-device account
    // tracking dead on Warren).
    single { SplashCompleteRepository() }
    single {
        AutoStartAndConnectOnBootRepository(
            get(),
            get(named(BOOT_COMPLETED_RECEIVER_COMPONENT_NAME)),
        )
    }
    single { WireguardConstraintsRepository(get()) }

    // D.4 step 38: AccountExpiryInAppNotificationUseCase dropped (subscription dead).
    single { TunnelStateNotificationUseCase(get(), get(), get()) } bind
        InAppNotificationUseCase::class
    single {
        VersionNotificationUseCase(get(), BuildConfig.ENABLE_IN_APP_VERSION_NOTIFICATIONS)
    } bind InAppNotificationUseCase::class
    // D.4 step 41: NewDeviceNotificationUseCase dropped.
    single { NewChangelogNotificationUseCase(get()) } bind InAppNotificationUseCase::class
    if (Build.VERSION.SDK_INT == Build.VERSION_CODES.BAKLAVA) {
        single { Android16UpdateWarningUseCase(get(), get()) } bind InAppNotificationUseCase::class
    }

    // D.4 step 37: OutOfTimeUseCase dropped (Mullvad subscription expiry
    // model dead on Warren).
    // D.4 step 44: InternetAvailableUseCase + SupportEmailUseCase +
    // SelectedLocationUseCase + SelectSinglehopUseCase + ModifyMultihopUseCase
    // + SelectAndEnableMultihopUseCase + ModifyAndEnableMultihopUseCase all
    // dropped — orphan koin singles, no consumer outside their own files +
    // tests (deleted CustomList/SelectLocation/Multihop screens, plus
    // ReportProblemViewModel no longer wires SupportEmailUseCase).
    single { SystemVpnSettingsAvailableUseCase(androidContext()) }
    // D.4 step 29: CustomList* + FilterChip + Filtered/Selected relay use
    // cases removed - all consumers were the deleted Mullvad
    // SelectLocation/CustomList screens. SelectedLocationTitleUseCase +
    // LastKnownLocationUseCase + ProviderToOwnershipsUseCase + their
    // dependents kept because ConnectViewModel still references them.
    single { SelectedLocationTitleUseCase(get()) }
    single { LastKnownLocationUseCase(get()) }
    single { DeleteCustomDnsUseCase(get()) }

    single { InAppNotificationController(getAll(), MainScope()) }

    single { ChangelogDataProvider(get()) }

    // D.4 step 36: PaymentProvider + PaymentLogic + PlayPaymentLogic /
    // EmptyPaymentUseCase dropped (Mullvad Play Store billing dead on
    // Warren — BIP39 wallet identity replaces VPN subscriptions).

    single { AppVersionInfoRepository(get(), get()) }

    // D.4 step 27: RelayListScrollConnection (Mullvad relay-list scroll position
    // for SelectLocationScreen) removed - SelectLocation is dead.

    // View models (D.4 step 16: AccountViewModel + DeleteAccountConfirmation +
    // ManageDevicesViewModel + VoucherDialogViewModel + AddTimeViewModel
    // registrations removed; the corresponding Mullvad-legacy screens are no
    // longer in the navigation graph)
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
            isPlayBuild = IS_PLAY_BUILD,
            resolveAppListing = get(),
        )
    }
    // D.4 step 28: DeviceListViewModel + LoginViewModel removed
    // (Mullvad multi-device + account-number flows, replaced by
    // WarrenWalletViewModel).
    viewModel { DeviceRevokedViewModel(get(), get()) }
    // D.4 step 53: MtuDialogViewModel + DnsDialogViewModel dropped (VpnSettings
    // module deleted — Mullvad daemon settings sync dead).
    viewModel { WarrenWalletViewModel(get()) }
    viewModel { PrivacyDisclaimerViewModel(get(), IS_PLAY_BUILD) }
    // D.4 step 27: SelectLocationViewModel removed (Mullvad relay-list
    // picker, replaced by WarrenLocationPicker).
    viewModel { SettingsViewModel(get(), get(), get(), IS_PLAY_BUILD) }
    viewModel { SplashViewModel(get(), get(), get()) }
    // D.4 step 53: VpnSettingsViewModel dropped (module deleted).
    viewModel {
        ReportProblemViewModel(
            warrenProblemReporter = get(),
            problemReportRepository = get(),
            accountRepository = get(),
            isPlayBuild = IS_PLAY_BUILD,
        )
    }
    viewModel { ViewLogsViewModel(get()) }
    // D.4 step 27: Filter + CustomList ViewModels removed (only reached
    // from dead SelectLocationScreen).
    // D.4 step 35: ServerIpOverrides + ResetServerIpOverridesConfirmation
    // ViewModels removed (Mullvad relay-IP override is dead on Warren).
    // D.4 step 33: ApiAccess ViewModels removed (5 VMs : List, Edit,
    // Save, Details, DeleteConfirmation). Warren API endpoint is
    // hardcoded - no per-user access method configuration.
    // D.4 step 34: anticensorship VMs removed (AntiCensorshipSettings +
    // CustomPortDialog + SelectPort - all Mullvad WG-over-X transport
    // features ; Warren uses native Quinn + M4.0 toggle).
    // D.4 step 32: MultihopViewModel removed (multihop now configured
    // via WarrenTunnelSettings toggles).
    viewModel { NotificationSettingsViewModel(get()) }
    // D.4 step 27: SearchLocation + SelectLocationList ViewModels removed
    // (Mullvad relay-list picker, replaced by WarrenLocationPicker).
    // D.4 step 32: DaitaViewModel removed (DAITA now toggled via
    // WarrenTunnelSettings).
    // D.4 step 28: ApiUnreachableViewModel removed - the
    // ApiUnreachable screen was reached only from Mullvad LoginScreen,
    // which is gone.
    // D.4 step 27: LocationBottomSheetViewModel removed (Mullvad
    // location bottom-sheet from SelectLocationScreen).
    viewModel { AppIconViewModel(get()) }
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
private val IS_PLAY_BUILD = BuildConfig.FLAVOR_billing == BillingTypes.PLAY
