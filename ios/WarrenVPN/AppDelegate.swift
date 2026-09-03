//
//  AppDelegate.swift
//  MullvadVPN
//
//  Created by pronebird on 19/03/2019.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import BackgroundTasks
import WarrenLogging
import WarrenMockData
import WarrenREST
import WarrenRustRuntime
import WarrenSettings
import WarrenTypes
import Operations
import UIKit
import UserNotifications

@main
class AppDelegate: UIResponder, UIApplicationDelegate, UNUserNotificationCenterDelegate, @unchecked Sendable {
    nonisolated(unsafe) private var logger: Logger!

    #if targetEnvironment(simulator)
        private var simulatorTunnelProviderHost: SimulatorTunnelProviderHost?
    #endif

    private let operationQueue = AsyncOperationQueue.makeSerial()

    private(set) var tunnelStore: TunnelStore!
    nonisolated(unsafe) private(set) var tunnelManager: TunnelManager!
    nonisolated(unsafe) private(set) var storePaymentManager: StorePaymentManager!

    private var proxyFactory: ProxyFactoryProtocol!
    private(set) var apiProxy: APIQuerying!

    private(set) var addressCacheUpdateScheduler: AddressCacheUpdateScheduler!
    nonisolated(unsafe) private(set) var relayCacheTracker: RelayCacheTracker!
    nonisolated(unsafe) private var apiTransportMonitor: APITransportMonitor!
    private var settingsObserver: TunnelBlockObserver!
    private var migrationManager: MigrationManager!

    nonisolated(unsafe) private(set) var accessMethodRepository = AccessMethodRepository(
        shadowsocksCiphers: ShadowsocksCipherService().getCiphers()
    )
    nonisolated(unsafe) private(set) var appPreferences = AppPreferences()
    private(set) var shadowsocksLoader: ShadowsocksLoader!
    private(set) var ipOverrideRepository = IPOverrideRepository()
    private(set) var relaySelector: RelaySelectorWrapper!
    private(set) var launchArguments = LaunchArguments()
    var apiContext: MullvadApiContext!
    var accessMethodReceiver: MullvadAccessMethodReceiver!
    private var shadowsocksCacheCleaner: ShadowsocksCacheCleaner!
    let breadcrumbsProvider = BreadcrumbsProvider()
    /// The community-forum wallet login (doc 55), from a deep link or a typed
    /// code; the scene that owns the window supplies its presenter.
    let forumLogin = WarrenForumLoginFlow()
    /// Coexistence: this build stands down when a higher-priority product
    /// environment is installed beside it. Nil on prod, which outranks
    /// everything and watches nothing.
    nonisolated(unsafe) private var envStandDown: WarrenEnvStandDown?
    /// The launch announcements this installation holds, and the foreground
    /// poll that keeps them current.
    nonisolated(unsafe) private var launchAnnouncements: WarrenLaunchAnnouncements?
    /// The view controller the full announcement is presented on, resolved at
    /// presentation time so the sheet lands over whatever is on screen. The
    /// scene that owns the window supplies it, like the forum flow's.
    nonisolated(unsafe) var announcementPresenter: (() -> UIViewController?)?

    let notificationSettingsListener = NotificationSettingsListener()
    private var notificationSettingsUpdater: NotificationSettingsUpdater!

    // MARK: - Application lifecycle

    func application(
        _ application: UIApplication,
        didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?
    ) -> Bool {
        application.accessibilityLanguage = Locale.current.language.languageCode?.identifier

        // Plain UINavigationControllers (wallet login/backup flows) would
        // otherwise render bar buttons in system blue, which clashes with
        // the Warren palette. CustomNavigationController still applies its
        // own instance tint, and alert controllers keep their own tint.
        UINavigationBar.appearance().tintColor = .Warren.yellow

        if let overriddenLaunchArguments = try? ProcessInfo.processInfo.decode(LaunchArguments.self) {
            launchArguments = overriddenLaunchArguments
        }

        let containerURL = ApplicationConfiguration.containerURL
        migrationManager = MigrationManager(cacheDirectory: containerURL)
        configureLogging()

        let ipOverrideWrapper = IPOverrideWrapper(
            relayCache: RelayCache(cacheDirectory: containerURL),
            ipOverrideRepository: ipOverrideRepository
        )

        let tunnelSettingsListener = TunnelSettingsListener()
        let tunnelSettingsUpdater = SettingsUpdater(listener: tunnelSettingsListener)

        notificationSettingsUpdater = NotificationSettingsUpdater(listener: notificationSettingsListener)

        let shadowsocksCache = ShadowsocksConfigurationCache(cacheDirectory: containerURL)
        let shadowsocksRelaySelector = ShadowsocksRelaySelector(
            relayCache: ipOverrideWrapper
        )

        let tunnelSettings = (try? SettingsManager.readSettings()) ?? LatestTunnelSettings()

        shadowsocksLoader = ShadowsocksLoader(
            cache: shadowsocksCache,
            relaySelector: shadowsocksRelaySelector,
            tunnelSettings: tunnelSettings,
            settingsUpdater: tunnelSettingsUpdater
        )

        shadowsocksCacheCleaner = ShadowsocksCacheCleaner(cache: shadowsocksCache)

        let opaqueAccessMethodSettingsWrapper = initAccessMethodSettingsWrapper(
            methods: accessMethodRepository.fetchAll()
        )

        // swift-format-ignore: NeverUseForceTry
        apiContext = try! MullvadApiContext(
            host: REST.defaultAPIHostname,
            address: REST.defaultAPIEndpoint.description,
            encryptedDnsDomain: REST.encryptedDNSHostname,
            domainFrontingFront: REST.domainFrontingFront,
            domainFrontingProxyHost: REST.domainFrontingProxyHost,
            shadowsocksProvider: shadowsocksLoader,
            accessMethodWrapper: opaqueAccessMethodSettingsWrapper,
            accessMethodChangeListeners: [accessMethodRepository, shadowsocksCacheCleaner]
        )

        accessMethodReceiver = MullvadAccessMethodReceiver(
            apiContext: apiContext,
            validShadowsocksCiphers: accessMethodRepository.shadowsocksCiphers,
            accessMethodsDataSource: accessMethodRepository.accessMethodsPublisher,
            requestDataSource: accessMethodRepository.requestAccessMethodPublisher
        )

        setUpProxies(containerURL: containerURL)
        let backgroundTaskProvider = BackgroundTaskProvider(
            backgroundTimeRemaining: application.backgroundTimeRemaining,
            application: application
        )

        relayCacheTracker = RelayCacheTracker(
            relayCache: ipOverrideWrapper,
            backgroundTaskProvider: backgroundTaskProvider,
            apiProxy: apiProxy
        )

        addressCacheUpdateScheduler = AddressCacheUpdateScheduler(
            backgroundTaskProvider: backgroundTaskProvider,
            apiProxy: apiProxy,
            apiContext: apiContext
        )

        tunnelStore = TunnelStore(application: backgroundTaskProvider)

        relaySelector = RelaySelectorWrapper(
            relayCache: ipOverrideWrapper
        )
        tunnelManager = createTunnelManager(
            backgroundTaskProvider: backgroundTaskProvider,
            relaySelector: relaySelector
        )
        // Coexistence: the tunnel refuses to arm while this build has stood
        // down for a higher-priority environment, and it reads the very record
        // the banner and the connect screen read, so the three cannot disagree.
        tunnelManager.envStandDownStore = appPreferences

        settingsObserver = TunnelBlockObserver(didUpdateTunnelSettings: { _, settings in
            tunnelSettingsListener.onNewSettings?(settings)
        })
        tunnelManager.addObserver(settingsObserver)

        // Warren has no Mullvad account number: the StoreKit credit goes
        // through warren-api signed with the wallet key (the interactor
        // talks to WarrenWalletInteractor), NOT the deleted apiProxy /
        // accountProxy.
        storePaymentManager = StorePaymentManager(
            interactor: StorePaymentManagerInteractor(tunnelManager: tunnelManager)
        )

        let apiRequestFactory = MullvadApiRequestFactory(
            apiContext: apiContext,
            encoder: REST.Coding.makeJSONEncoder()
        )
        let apiTransportProvider = APITransportProvider(requestFactory: apiRequestFactory)

        apiTransportMonitor = APITransportMonitor(
            tunnelManager: tunnelManager,
            tunnelStore: tunnelStore,
            requestFactory: apiRequestFactory
        )

        setUpSimulatorHost(
            apiTransportProvider: apiTransportProvider,
            relaySelector: relaySelector
        )
        registerBackgroundTasks()
        setupNotifications(
            tunnelSettings: tunnelSettings,
            tunnelSettingsUpdater: tunnelSettingsUpdater
        )
        addApplicationNotifications(application: application)
        startInitialization(application: application)

        // Pre-warm @Observable infrastructure for LocationNode to avoid first-render lag
        // in SelectLocationView. SwiftUI's observation system has initialization overhead
        // that is cached after first use.
        DispatchQueue.global(qos: .userInitiated).async {
            _ = LocationNode(name: "", code: "")
        }

        return true
    }

    private func createTunnelManager(
        backgroundTaskProvider: BackgroundTaskProviding,
        relaySelector: RelaySelectorProtocol
    ) -> TunnelManager {
        return TunnelManager(
            backgroundTaskProvider: backgroundTaskProvider,
            tunnelStore: tunnelStore,
            relayCacheTracker: relayCacheTracker,
            apiProxy: apiProxy,
            relaySelector: relaySelector
        )
    }

    private func setUpProxies(containerURL: URL) {
        if launchArguments.target == .screenshots {
            proxyFactory = MockProxyFactory.makeProxyFactory(
                apiTransportProvider: REST.AnyAPITransportProvider { [weak self] in
                    self?.apiTransportMonitor.makeTransport()
                }
            )
        } else {
            proxyFactory = REST.ProxyFactory.makeProxyFactory(
                apiTransportProvider: REST.AnyAPITransportProvider { [weak self] in
                    self?.apiTransportMonitor.makeTransport()
                }
            )
        }
        apiProxy = proxyFactory.createAPIProxy()
    }

    private func setUpSimulatorHost(
        apiTransportProvider: APITransportProvider,
        relaySelector: RelaySelectorWrapper
    ) {
        #if targetEnvironment(simulator)
            // Configure mock tunnel provider on simulator
            simulatorTunnelProviderHost = SimulatorTunnelProviderHost(
                relaySelector: relaySelector,
                apiTransportProvider: apiTransportProvider
            )
            SimulatorTunnelProvider.shared.delegate = simulatorTunnelProviderHost
        #endif
    }

    // MARK: - UISceneSession lifecycle

    func application(
        _ application: UIApplication,
        configurationForConnecting connectingSceneSession: UISceneSession,
        options: UIScene.ConnectionOptions
    ) -> UISceneConfiguration {
        let sceneConfiguration = UISceneConfiguration(
            name: "Default Configuration",
            sessionRole: connectingSceneSession.role
        )
        sceneConfiguration.delegateClass = SceneDelegate.self

        return sceneConfiguration
    }

    func application(
        _ application: UIApplication,
        didDiscardSceneSessions sceneSessions: Set<UISceneSession>
    ) {}

    // MARK: - Notifications

    @objc private func didBecomeActive(_ notification: Notification) {
        relayCacheTracker.startPeriodicUpdates()
        addressCacheUpdateScheduler.startPeriodicUpdates()
        // The re-check on every activation is what catches up on whatever the
        // operator published while the app was away; the poll it starts keeps
        // a long foreground session current.
        launchAnnouncements?.startPolling()
    }

    @objc private func willResignActive(_ notification: Notification) {
        relayCacheTracker.stopPeriodicUpdates()
        addressCacheUpdateScheduler.stopPeriodicUpdates()
        // Nothing is fetched from the background: a background cadence would
        // make the app a periodic beacon for a card nobody is looking at.
        launchAnnouncements?.stopPolling()
    }

    @objc private func didEnterBackground(_ notification: Notification) {
        scheduleBackgroundTasks()
    }

    // MARK: - Background tasks

    private func registerBackgroundTasks() {
        registerAppRefreshTask()
        registerAddressCacheUpdateTask()
    }

    private func registerAppRefreshTask() {
        let isRegistered = BGTaskScheduler.shared.register(
            forTaskWithIdentifier: BackgroundTask.appRefresh.identifier,
            using: .main
        ) { [self] task in
            nonisolated(unsafe) let handle = relayCacheTracker.updateRelays { result in
                task.setTaskCompleted(success: result.isSuccess)
            }

            task.expirationHandler = { @Sendable in
                handle.cancel()
            }

            scheduleAppRefreshTask()
        }

        if isRegistered {
            logger.debug("Registered app refresh task.")
        } else {
            logger.error("Failed to register app refresh task.")
        }
    }

    private func registerAddressCacheUpdateTask() {
        let isRegistered = BGTaskScheduler.shared.register(
            forTaskWithIdentifier: BackgroundTask.addressCacheUpdate.identifier,
            using: .main
        ) { [self] task in
            addressCacheUpdateScheduler.updateEndpoints { [self] result in
                scheduleAddressCacheUpdateTask()
                task.setTaskCompleted(success: result.isSuccess)
            }
        }

        if isRegistered {
            logger.debug("Registered address cache update task.")
        } else {
            logger.error("Failed to register address cache update task.")
        }
    }

    private func scheduleBackgroundTasks() {
        scheduleAppRefreshTask()
        scheduleAddressCacheUpdateTask()
    }

    private func scheduleAppRefreshTask() {
        do {
            let date = relayCacheTracker.getNextUpdateDate()

            let request = BGAppRefreshTaskRequest(identifier: BackgroundTask.appRefresh.identifier)
            request.earliestBeginDate = date

            logger.debug("Schedule app refresh task at \(date.logFormatted).")

            try BGTaskScheduler.shared.submit(request)
        } catch {
            logger.error(error: error, message: "Could not schedule app refresh task.")
        }
    }

    private func scheduleAddressCacheUpdateTask() {
        do {
            let date = addressCacheUpdateScheduler.nextScheduleDate()

            let request = BGProcessingTaskRequest(identifier: BackgroundTask.addressCacheUpdate.identifier)
            request.requiresNetworkConnectivity = true
            request.earliestBeginDate = date

            logger.debug("Schedule address cache update task at \(date.logFormatted).")

            try BGTaskScheduler.shared.submit(request)
        } catch {
            logger.error(error: error, message: "Could not schedule address cache update task.")
        }
    }

    // MARK: - Private

    private func configureLogging() {
        let header = "WarrenVPN version \(Bundle.main.productVersion)"
        let loggerBuilder = LoggerBuilder.shared

        loggerBuilder.addFileOutput(
            fileURL: ApplicationConfiguration.newLogFileURL(for: .mainApp, in: ApplicationConfiguration.containerURL),
            header: header
        )
        #if DEBUG
            loggerBuilder.addOSLogOutput(subsystem: ApplicationTarget.mainApp.bundleIdentifier)
        #endif
        loggerBuilder.install()

        // Initialize Rust logging to forward to Swift Logger
        RustLogging.initialize()

        logger = Logger(label: "AppDelegate")
    }

    private func addApplicationNotifications(application: UIApplication) {
        let notificationCenter = NotificationCenter.default

        notificationCenter.addObserver(
            self,
            selector: #selector(didBecomeActive(_:)),
            name: UIApplication.didBecomeActiveNotification,
            object: application
        )
        notificationCenter.addObserver(
            self,
            selector: #selector(willResignActive(_:)),
            name: UIApplication.willResignActiveNotification,
            object: application
        )
        notificationCenter.addObserver(
            self,
            selector: #selector(didEnterBackground(_:)),
            name: UIApplication.didEnterBackgroundNotification,
            object: application
        )
    }

    private func setupNotifications(
        tunnelSettings: LatestTunnelSettings,
        tunnelSettingsUpdater: SettingsUpdater

    ) {
        let appVersionService = AppVersionService(
            urlSession: URLSession.shared,
            appPreferences: appPreferences,
            mainAppBundleIdentifier: ApplicationTarget.mainApp.bundleIdentifier
        )

        NotificationManager.shared.notificationProviders = [
            LatestChangesNotificationProvider(appPreferences: appPreferences),
            TunnelStatusNotificationProvider(tunnelManager: tunnelManager),
            AccountExpirySystemNotificationProvider(
                isNotificationEnabled: appPreferences.notificationSettings.isAccountNotificationEnabled,
                notificationSettingsUpdater: notificationSettingsUpdater, tunnelManager: tunnelManager),
            AccountExpiryInAppNotificationProvider(tunnelManager: tunnelManager),
            NewAppVersionInAppNotificationProvider(
                tunnelManager: tunnelManager,
                appVersionService: appVersionService
            ),
            WarrenFailoverNotificationProvider(acknowledgeStore: appPreferences),
            makeEnvStandDownNotificationProvider(),
            makeAnnouncementNotificationProvider(),
        ]
        UNUserNotificationCenter.current().delegate = self
    }

    /// The launch-announcement feed and its banner. Built together so the
    /// notifications are re-evaluated the moment the held set moves, which is
    /// the only thing that changes what the banner shows.
    private func makeAnnouncementNotificationProvider() -> WarrenAnnouncementNotificationProvider {
        let feed = WarrenLaunchAnnouncements(
            backend: WarrenLaunchAnnouncements.Backend(
                fetch: { url in
                    var request = URLRequest(url: url)
                    request.timeoutInterval = 15
                    guard let (data, response) = try? await URLSession.shared.data(for: request),
                        (response as? HTTPURLResponse)?.statusCode == 200
                    else {
                        return nil
                    }
                    return data
                },
                verify: { body, version in
                    WarrenAnnouncementsVerifier.verify(envelope: body, currentVersion: version)
                },
                address: {
                    // Reading the wallet derives a seed from the phrase, which
                    // is the same cost the lookup itself pays, so it goes on
                    // the same queue rather than on a thread of the
                    // cooperative pool.
                    await withCheckedContinuation {
                        (continuation: CheckedContinuation<String?, Never>) in
                        Self.campaignVoucherQueue.async {
                            continuation.resume(returning: Self.walletAddress())
                        }
                    }
                },
                voucher: { campaignID in
                    // The lookup signs and blocks on an HTTP round trip, so it
                    // runs on a queue of its own rather than parking a thread
                    // of the cooperative pool for the length of a request.
                    await withCheckedContinuation {
                        (continuation: CheckedContinuation<WarrenCampaignVoucherAnswer, Never>) in
                        Self.campaignVoucherQueue.async {
                            continuation.resume(returning: Self.campaignVoucher(campaignID))
                        }
                    }
                }
            )
        )
        launchAnnouncements = feed

        let provider = WarrenAnnouncementNotificationProvider(
            source: { [weak feed] in feed?.announcements ?? [] },
            dismissStore: appPreferences,
            present: { [weak self] announcement in
                MainActor.assumeIsolated {
                    guard let presenter = self?.announcementPresenter?() else { return }
                    WarrenAnnouncementPresenter.present(announcement, over: presenter)
                }
            }
        )
        feed.didChange = {
            // `updateNotifications` asserts it runs on the main queue, and the
            // poll that moved the set does not.
            DispatchQueue.main.async { NotificationManager.shared.updateNotifications() }
        }
        return provider
    }

    /// Where the blocking, wallet-signed campaign lookup runs.
    private static let campaignVoucherQueue = DispatchQueue(
        label: "WarrenCampaignVoucher",
        qos: .utility
    )

    /// Warren SS58 address of the wallet the campaign lookup signs with, `nil`
    /// when this device holds none. It keys the codes the feed holds so one
    /// account is never shown the offer drawn for another, and it is never
    /// logged.
    private static func walletAddress() -> String? {
        guard let mnemonic = try? WarrenWalletKeychain.load(),
            let wallet = try? WarrenWallet.fromMnemonic(mnemonic)
        else {
            return nil
        }
        defer { wallet.forgetSecret() }
        return wallet.publicKeyAddress
    }

    /// This account's code for `campaignID`, over the wallet-signed lookup.
    ///
    /// The three outcomes are kept apart because the feed holds a server
    /// answer and re-asks a failure: reading a transient outage as "outside
    /// the cohort" would tell a cohort member they were never eligible for
    /// the rest of the session.
    ///
    /// The wallet is read the way the forum login reads it, and the code is
    /// never logged, not even as a length: it is a bearer token worth a month
    /// of service and its only destination is the reader's own screen.
    private static func campaignVoucher(_ campaignID: String) -> WarrenCampaignVoucherAnswer {
        guard let mnemonic = try? WarrenWalletKeychain.load(),
            let wallet = try? WarrenWallet.fromMnemonic(mnemonic)
        else {
            return .unanswered
        }
        defer { wallet.forgetSecret() }
        switch WarrenAccountClient.campaignVoucher(seed: wallet.seed, campaignID: campaignID) {
        case let .success(code):
            return code.map(WarrenCampaignVoucherAnswer.drawn) ?? .outside
        case .failure:
            return .unanswered
        }
    }

    /// The coexistence stand-down and its banner. Built together so the
    /// provider can be invalidated the moment the record moves, which is the
    /// only thing that changes what the banner shows.
    private func makeEnvStandDownNotificationProvider() -> WarrenEnvStandDownNotificationProvider {
        let standDown = WarrenEnvStandDown(
            store: appPreferences,
            device: WarrenEnvStandDown.Device(
                isInstalled: { scheme in
                    guard let url = URL(string: "\(scheme)://") else { return false }
                    return MainActor.assumeIsolated { UIApplication.shared.canOpenURL(url) }
                },
                stopTunnelAndClearOnDemand: { [weak self] completion in
                    guard let self else {
                        completion()
                        return
                    }
                    tunnelManager.stopTunnel(isOnDemandEnabled: false) { _ in completion() }
                },
                readKillSwitch: { [weak self] in
                    self?.tunnelManager.settings.includeAllNetworks.includeAllNetworksIsEnabled ?? false
                },
                writeKillSwitch: { [weak self] isOn in
                    guard let self else { return }
                    var settings = tunnelManager.settings.includeAllNetworks
                    settings.includeAllNetworksState = isOn ? .on : .off
                    tunnelManager.updateSettings([.includeAllNetworks(settings)])
                }
            )
        )
        envStandDown = standDown

        let provider = WarrenEnvStandDownNotificationProvider(
            store: appPreferences,
            confirm: { [weak standDown] in standDown?.confirmStandDown() },
            reEnable: { [weak standDown] in standDown?.reEnable() }
        )
        standDown.didChange = { [weak provider] in provider?.invalidate() }
        return provider
    }

    private func startInitialization(application: UIApplication) {
        let defaultLocationOperation = getDefaultLocationOperation()
        let loadTunnelStoreOperation = getLoadTunnelStoreOperation()
        let initTunnelManagerOperation = getInitTunnelManagerOperation()

        var operations: [Operation] = [
            defaultLocationOperation,
            loadTunnelStoreOperation,
        ]

        let wipeSettingsOperation = getWipeSettingsOperation()
        let migrateSettingsOperation = getMigrateSettingsOperation(application: application)

        // Dependencies
        defaultLocationOperation.addDependency(wipeSettingsOperation)
        migrateSettingsOperation.addDependencies([
            wipeSettingsOperation,
            loadTunnelStoreOperation,
        ])
        initTunnelManagerOperation.addDependency(migrateSettingsOperation)

        operations.append(contentsOf: [
            wipeSettingsOperation,
            migrateSettingsOperation,
            initTunnelManagerOperation,
        ])

        operationQueue.addOperations(operations, waitUntilFinished: false)
    }

    private func getLoadTunnelStoreOperation() -> AsyncBlockOperation {
        AsyncBlockOperation(dispatchQueue: .main) { [self] finish in
            MainActor.assumeIsolated {
                tunnelStore.loadPersistentTunnels { [self] error in
                    if let error {
                        logger.error(
                            error: error,
                            message: "Failed to load persistent tunnels."
                        )
                    }
                    finish(nil)
                }
            }
        }
    }

    private func getMigrateSettingsOperation(application: UIApplication) -> AsyncBlockOperation {
        AsyncBlockOperation(
            dispatchQueue: .main,
            block: { [self] (finish: @escaping @Sendable (Error?) -> Void) in
                MainActor.assumeIsolated {
                    migrationManager
                        .migrateSettings(store: SettingsManager.store) { [self] migrationResult in
                            switch migrationResult {
                            case .success:
                                // Tell the tunnel to re-read tunnel configuration after migration.
                                logger.debug("Successful migration from UI Process")
                                tunnelManager.reconnectTunnel(selectNewRelay: true)
                                fallthrough

                            case .nothing:
                                logger.debug("Attempted migration from UI Process, but found nothing to do")
                                finish(nil)

                            case let .failure(error):
                                logger.error("Failed migration from UI Process: \(error)")
                                MainActor.assumeIsolated {
                                    let migrationUIHandler =
                                        application.connectedScenes
                                        .first { $0 is SettingsMigrationUIHandler } as? SettingsMigrationUIHandler

                                    if let migrationUIHandler {
                                        migrationUIHandler.showMigrationError(error) {
                                            MainActor.assumeIsolated {
                                                finish(error)
                                            }
                                        }
                                    } else {
                                        finish(error)
                                    }
                                }
                            }
                        }
                }
            }
        )
    }

    private func getInitTunnelManagerOperation() -> AsyncBlockOperation {
        // This operation is always treated as successful no matter what the configuration load yields.
        // If the tunnel settings or device state can't be read, we simply pretend they are not there
        // and leave user in logged out state. VPN config will be removed as well.
        AsyncBlockOperation(dispatchQueue: .main) { [weak self] finish in
            guard let self else {
                finish(nil)
                return
            }
            self.tunnelManager.loadConfiguration {
                self.logger.debug("Finished initialization.")

                // Observe the device once the tunnel is loaded: the stand-down
                // stops the tunnel and reads the settings, and neither answers
                // before this point.
                self.envStandDown?.refresh()

                NotificationManager.shared.updateNotifications()

                Task {
                    await self.storePaymentManager.start()
                    finish(nil)
                }
            }
        }
    }

    /// Returns an operation that acts on the following conditions:
    /// 1. If the app has never been launched, preload with default settings.
    /// - or -
    /// 1. Has the app been launched at least once after install? (`FirstTimeLaunch.hasFinished`)
    /// 2. Has the app - at some point in time - been updated from a version compatible with wiping settings?
    /// (`SettingsManager.getShouldWipeSettings()`)
    /// If (1) is `false` and (2) is `true`, we know that the app has been freshly installed/reinstalled and is
    /// compatible, thus triggering a settings wipe.
    private func getWipeSettingsOperation() -> AsyncBlockOperation {
        AsyncBlockOperation {
            let appHasNeverBeenLaunched =
                !self.appPreferences.hasDoneFirstTimeLaunch && !SettingsManager.getShouldWipeSettings()
            let appWasLaunchedAfterReinstall =
                !self.appPreferences.hasDoneFirstTimeLaunch && SettingsManager.getShouldWipeSettings()

            if appHasNeverBeenLaunched {
                try? SettingsManager.writeSettings(LatestTunnelSettings())
            } else if appWasLaunchedAfterReinstall {
                // Warren has no device registry to revoke on reinstall; the
                // wallet identity lives in the Keychain and the exit only
                // tracks ephemeral sessions.
                SettingsManager.resetStore(policy: .all)
                try? SettingsManager.writeSettings(LatestTunnelSettings())

                // Default access methods need to be repopulated again after settings wipe.
                self.accessMethodRepository.addDefaultsMethods()
                // At app startup, the relay cache tracker will get populated with a list of overriden IPs.
                // The overriden IPs will get wiped, therefore, the cache needs to be pruned as well.
                try? self.relayCacheTracker.refreshCachedRelays()
            }

            SettingsManager.setShouldWipeSettings()
        }
    }

    private func getDefaultLocationOperation() -> AsyncBlockOperation {
        AsyncBlockOperation {
            guard !self.appPreferences.hasDoneFirstTimeLaunch else {
                return
            }
            self.appPreferences.hasDoneFirstTimeLaunch = true

            // No need to keep the handle since we're not waiting or cancelling the completion anyway.
            _ = self.relayCacheTracker.updateRelays { _ in
                Task {
                    guard let cachedRelays = try? self.relayCacheTracker.getCachedRelays() else {
                        return
                    }

                    let locationService = DefaultLocationService(
                        urlSession: URLSession.shared, relayCache: cachedRelays)
                    let locationIdentifier = try? await locationService.fetchCurrentLocationIdentifier()
                    let userSelectedRelays: UserSelectedRelays =
                        if let country = locationIdentifier?.country {
                            UserSelectedRelays(locations: [.country(country)])
                        } else {
                            .default
                        }

                    let constraint = RelayConstraint.only(userSelectedRelays)

                    if !self.appPreferences.hasDoneFirstTimeLogin {
                        self.tunnelManager.updateSettings([
                            .relayConstraints(RelayConstraints(entryLocations: constraint, exitLocations: constraint))
                        ])
                    }
                }
            }
        }
    }

    // MARK: - UNUserNotificationCenterDelegate

    nonisolated func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        didReceive response: UNNotificationResponse,
        withCompletionHandler completionHandler: @escaping () -> Void
    ) {
        nonisolated(unsafe) let nonisolatedResponse = response
        nonisolated(unsafe) let nonisolatedCompletionHandler = completionHandler

        let blockOperation = AsyncBlockOperation(dispatchQueue: .main) {
            NotificationManager.shared.handleSystemNotificationResponse(nonisolatedResponse)

            nonisolatedCompletionHandler()
        }

        operationQueue.addOperation(blockOperation)
    }

    nonisolated func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        willPresent notification: UNNotification,
        withCompletionHandler completionHandler: @escaping (UNNotificationPresentationOptions) -> Void
    ) {
        completionHandler([.list, .banner, .sound])
    }
}
