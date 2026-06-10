//
//  PacketTunnelProvider.swift
//  PacketTunnel
//
//  Created by pronebird on 31/08/2023.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import Foundation
import WarrenLogging
import WarrenREST
import WarrenRustRuntime
import WarrenSettings
import WarrenTypes
@preconcurrency import NetworkExtension
import PacketTunnelCore

class PacketTunnelProvider: NEPacketTunnelProvider, @unchecked Sendable {
    private let internalQueue = DispatchQueue(label: "PacketTunnel-internalQueue")
    private let providerLogger: Logger

    /// The selected tunnel implementation (Warren Quinn, or GotaTun in debug builds).
    private var implementation: TunnelImplementation!
    private var appMessageHandler: AppMessageHandler!
    private var deviceChecker: DeviceChecker!
    private var newAppVersionSystemNoticationHandler: NewAppVersionSystemNotificationHandler!
    private let tunnelSettingsUpdater: SettingsUpdater
    private var migrationManager: MigrationManager
    let migrationFailureIterator = REST.RetryStrategy.failedMigrationRecovery.makeDelayIterator()

    private let tunnelSettingsListener = TunnelSettingsListener()

    var apiContext: MullvadApiContext!
    var accessMethodReceiver: MullvadAccessMethodReceiver!
    private var shadowsocksCacheCleaner: ShadowsocksCacheCleaner!

    override init() {
        Self.configureLogging()
        providerLogger = Logger(label: "PacketTunnelProvider")
        providerLogger.info("Starting new packet tunnel")

        let containerURL = ApplicationConfiguration.containerURL

        let ipOverrideWrapper = IPOverrideWrapper(
            relayCache: RelayCache(cacheDirectory: containerURL),
            ipOverrideRepository: IPOverrideRepository()
        )
        tunnelSettingsUpdater = SettingsUpdater(listener: tunnelSettingsListener)
        migrationManager = MigrationManager(cacheDirectory: containerURL)

        super.init()

        performSettingsMigration()

        let settingsReader = TunnelSettingsManager(settingsReader: SettingsReader()) { [weak self] settings in
            guard let self = self else { return }
            tunnelSettingsListener.onNewSettings?(settings.tunnelSettings)
        }

        let tunnelSettings = (try? settingsReader.read().tunnelSettings) ?? LatestTunnelSettings()
        let accessMethodRepository = AccessMethodRepository(
            shadowsocksCiphers: ShadowsocksCipherService().getCiphers()
        )

        setUpApiContextAndAccessMethodReceiver(
            appContainerURL: containerURL,
            ipOverrideWrapper: ipOverrideWrapper,
            accessMethodRepository: accessMethodRepository,
            tunnelSettings: tunnelSettings
        )

        setUpAccessMethodReceiver(
            accessMethodRepository: accessMethodRepository
        )

        let apiTransportProvider = APITransportProvider(
            requestFactory: MullvadApiRequestFactory(
                apiContext: apiContext,
                encoder: REST.Coding.makeJSONEncoder()
            )
        )

        let proxyFactory = REST.ProxyFactory.makeProxyFactory(
            apiTransportProvider: apiTransportProvider
        )
        let accountsProxy = proxyFactory.createAccountsProxy()
        let devicesProxy = proxyFactory.createDevicesProxy()

        deviceChecker = DeviceChecker(accountsProxy: accountsProxy, devicesProxy: devicesProxy)

        // Warren tunnels via Quinn (`warren-tunnel`). The debug `GotaTun`
        // toggle is an alternate path for local development (no real
        // tunnel traffic, useful for UI/iOS lifecycle smoke tests).
        // Default = `WarrenQuinnTunnelImplementation`.
        #if DEBUG
            if PacketTunnelDebugSettings.useGotaTun {
                providerLogger.info("Using GotaTun implementation (debug)")
                implementation = GotaTunTunnelImplementation()
            } else {
                providerLogger.info("Using Warren Quinn implementation")
                implementation = WarrenQuinnTunnelImplementation()
            }
        #else
            implementation = WarrenQuinnTunnelImplementation()
        #endif

        implementation.setUp(
            provider: self,
            internalQueue: internalQueue,
            ipOverrideWrapper: ipOverrideWrapper,
            settingsReader: settingsReader,
            apiTransportProvider: apiTransportProvider
        )

        let apiRequestProxy = APIRequestProxy(
            dispatchQueue: internalQueue,
            transportProvider: apiTransportProvider
        )
        appMessageHandler = AppMessageHandler(
            packetTunnelActor: implementation.actor,
            apiRequestProxy: apiRequestProxy
        )

        newAppVersionSystemNoticationHandler = NewAppVersionSystemNotificationHandler(
            appVersionService: AppVersionService(
                urlSession: URLSession.shared,
                appPreferences: AppPreferences(),
                mainAppBundleIdentifier: ApplicationTarget.mainApp.bundleIdentifier
            ),
            settingsUpdater: tunnelSettingsUpdater,
            tunnelSettings: tunnelSettings
        )
    }

    override func startTunnel(
        options: [String: NSObject]? = nil,
        completionHandler: @escaping @Sendable ((any Error)?) -> Void
    ) {
        let startOptions = parseStartOptions(options ?? [:])

        // Run account and device diagnostics during tunnel startup.
        startDeviceCheck()

        setTunnelNetworkSettings(
            initialTunnelNetworkSettings(),
            completionHandler: { error in
                if let error {
                    self.providerLogger
                        .error(
                            "Failed to configure tunnel with initial config: \(error)"
                        )
                } else {
                    self.providerLogger.debug("Starting tunnel implementation after initial configuration is applied")
                    Task { await self.implementation.startTunnel(options: startOptions) }
                }
                self.internalQueue.async {
                    completionHandler(error)
                }
            })
    }

    override func stopTunnel(with reason: NEProviderStopReason) async {
        providerLogger.debug("stopTunnel: \(ProviderStopReasonWrapper(reason: reason))")

        await implementation.stopTunnel()
    }

    override func handleAppMessage(_ messageData: Data) async -> Data? {
        return await appMessageHandler.handleAppMessage(messageData)
    }

    override func sleep() async {
        await implementation.sleep()
    }

    override func wake() {
        implementation.wake()
    }

    private func performSettingsMigration() {
        nonisolated(unsafe) var hasNotMigrated = true
        repeat {
            migrationManager.migrateSettings(
                store: SettingsManager.store,
                migrationCompleted: { [unowned self] migrationResult in
                    switch migrationResult {
                    case .success:
                        providerLogger.debug("Successful migration from PacketTunnel")
                        hasNotMigrated = false
                    case .nothing:
                        hasNotMigrated = false
                        providerLogger.debug("Attempted migration from PacketTunnel, but found nothing to do")
                    case let .failure(error):
                        providerLogger
                            .error(
                                "Failed migration from PacketTunnel: \(error)"
                            )
                    }
                }
            )
            if hasNotMigrated {
                // `next` returns an Optional value, but this iterator is guaranteed to always have a next value
                guard let delay = migrationFailureIterator.next() else { continue }

                providerLogger.error("Retrying migration in \(delay.timeInterval) seconds")
                // Block the launch of the Packet Tunnel for as long as the settings migration fail.
                // The process watchdog introduced by iOS 17 will kill this process after 60 seconds.
                Thread.sleep(forTimeInterval: delay.timeInterval)
            }
        } while hasNotMigrated
    }

    private func setUpApiContextAndAccessMethodReceiver(
        appContainerURL: URL,
        ipOverrideWrapper: IPOverrideWrapper,
        accessMethodRepository: AccessMethodRepository,
        tunnelSettings: LatestTunnelSettings
    ) {
        let shadowsocksCache = ShadowsocksConfigurationCache(cacheDirectory: appContainerURL)

        let shadowsocksRelaySelector = ShadowsocksRelaySelector(
            relayCache: ipOverrideWrapper
        )

        let shadowsocksLoader = ShadowsocksLoader(
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
    }

    private func setUpAccessMethodReceiver(
        accessMethodRepository: AccessMethodRepository
    ) {
        accessMethodReceiver = MullvadAccessMethodReceiver(
            apiContext: apiContext,
            validShadowsocksCiphers: accessMethodRepository.shadowsocksCiphers,
            accessMethodsDataSource: accessMethodRepository.accessMethodsPublisher,
            requestDataSource: accessMethodRepository.requestAccessMethodPublisher
        )
    }

    private func initialTunnelNetworkSettings() -> NETunnelNetworkSettings {
        let settings = NEPacketTunnelNetworkSettings(
            tunnelRemoteAddress: "\(IPv4Address.loopback)"
        )

        // IPv4 settings
        let ipv4Settings = NEIPv4Settings(
            addresses: [LocalNetworkIPs.gatewayAddressIpV4.rawValue],
            subnetMasks: ["255.255.255.255"]
        )
        ipv4Settings.includedRoutes = [NEIPv4Route.default()]
        settings.ipv4Settings = ipv4Settings

        // IPv6 settings
        let ipv6Settings = NEIPv6Settings(
            addresses: [LocalNetworkIPs.gatewayAddressIpV6.rawValue],
            networkPrefixLengths: [128]
        )
        ipv6Settings.includedRoutes = [NEIPv6Route.default()]
        settings.ipv6Settings = ipv6Settings

        // Inner tunnel MTU. Warren tunnels QUIC over UDP/443; 1280 (the
        // IPv6 minimum) leaves comfortable headroom for the QUIC + UDP +
        // IP framing under a typical 1500-byte physical path and matches
        // the actor-path default (`TunnelAdapterProtocol.asTunnelSettings`).
        settings.mtu = NSNumber(value: 1280)

        // DNS resolvers honor the user's DNS choice (custom resolvers or a
        // content-blocking resolver), falling back to the Warren exit DNS
        // forwarder. `matchDomains = [""]` makes the tunnel resolver
        // authoritative for all domains so DNS cannot leak to the
        // underlying network's resolver while connected. All resolvers sit
        // behind the default route, so queries traverse the tunnel to the
        // exit regardless of which resolver is selected.
        let dnsSettings = NEDNSSettings(servers: warrenDNSResolvers())
        dnsSettings.matchDomains = [""]
        settings.dnsSettings = dnsSettings

        return settings
    }

    /// Resolves the DNS servers to advertise, honoring the user's DNS
    /// settings (custom / content-blocking) the same way desktop and
    /// Android do, and defaulting to the Warren exit DNS forwarder (the
    /// iOS analog of Android's `EXIT_DNS_RESOLVER`).
    private func warrenDNSResolvers() -> [String] {
        // The Warren exit DNS forwarder, used unless the user picked
        // custom resolvers or enabled content blocking.
        let warrenExitDNSResolver = "10.66.0.1"

        let tunnelSettings = (try? SettingsReader().read().tunnelSettings) ?? LatestTunnelSettings()
        let dns = tunnelSettings.dnsSettings

        if dns.effectiveEnableCustomDNS {
            let custom = Array(dns.customDNSDomains.prefix(DNSSettings.maxAllowedCustomDNSDomains))
            if !custom.isEmpty {
                return custom.map { "\($0)" }
            }
        } else if let blockingResolver = dns.blockingOptions.serverAddress {
            return ["\(blockingResolver)"]
        }
        return [warrenExitDNSResolver]
    }
}

extension PacketTunnelProvider {
    private static func configureLogging() {
        let loggerBuilder = LoggerBuilder.shared
        let header = "PacketTunnel version \(Bundle.main.productVersion)"

        loggerBuilder.addFileOutput(
            fileURL: ApplicationConfiguration.newLogFileURL(
                for: .packetTunnel,
                in: ApplicationConfiguration.containerURL
            ),
            header: header
        )
        #if DEBUG
            loggerBuilder.addOSLogOutput(subsystem: ApplicationTarget.packetTunnel.bundleIdentifier)
        #endif
        loggerBuilder.install()

        // Initialize Rust logging to forward to Swift Logger
        RustLogging.initialize()
    }

    private func parseStartOptions(_ options: [String: NSObject]) -> StartOptions {
        let tunnelOptions = PacketTunnelOptions(rawOptions: options)
        var parsedOptions = StartOptions(launchSource: tunnelOptions.isOnDemand() ? .onDemand : .app)

        do {
            if let selectedRelays = try tunnelOptions.getSelectedRelays() {
                parsedOptions.launchSource = .app
                parsedOptions.selectedRelays = selectedRelays
            } else if !tunnelOptions.isOnDemand() {
                parsedOptions.launchSource = .system
            }
        } catch {
            providerLogger.error(error: error, message: "Failed to decode relay selector result passed from the app.")
        }

        return parsedOptions
    }
}

// MARK: - Device check

extension PacketTunnelProvider {
    private func startDeviceCheck() {
        // Warren enforces subscription / access exit-side via the wallet
        // pubkey allowlist; there is no Mullvad account or device to
        // diagnose client-side. The Mullvad device check queries
        // account/device endpoints that warren-api does not serve and
        // would wrongly force a logout, so it is skipped on the Warren
        // Quinn path. The GotaTun debug path (Mullvad WireGuard) keeps it.
        guard !(implementation is WarrenQuinnTunnelImplementation) else { return }
        Task {
            await startDeviceCheckInner()
        }
    }

    private func startDeviceCheckInner() async {
        let result = await deviceChecker.start()

        switch result {
        case let .failure(error):
            switch error {
            case is DeviceCheckError:
                providerLogger.error("\(error.description) Forcing a log out")
                implementation.actor.setErrorState(reason: .deviceLoggedOut)
            default:
                providerLogger
                    .error(
                        "Device check encountered a network error: \(error.description)"
                    )
            }

        case let .success(deviceCheck):
            if let blockedStateReason = deviceCheck.blockedStateReason {
                providerLogger.error("Entering blocked state after unsuccessful device check: \(blockedStateReason)")
                implementation.actor.setErrorState(reason: blockedStateReason)
            }
        }
    }
}
