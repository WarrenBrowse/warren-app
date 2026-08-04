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
    private var newAppVersionSystemNoticationHandler: NewAppVersionSystemNotificationHandler!
    private let tunnelSettingsUpdater: SettingsUpdater
    private var migrationManager: MigrationManager
    let migrationFailureIterator = REST.RetryStrategy.failedMigrationRecovery.makeDelayIterator()

    private let tunnelSettingsListener = TunnelSettingsListener()

    var apiContext: MullvadApiContext!
    var accessMethodReceiver: MullvadAccessMethodReceiver!
    private var shadowsocksCacheCleaner: ShadowsocksCacheCleaner!

    /// NWPath observer feeding the actor's reachability, so the observed
    /// state can tell reconnecting-with-network from reconnecting-offline
    /// (the "no internet" treatment) and a network-return edge triggers an
    /// immediate fresh-socket redial. Started with the tunnel, stopped with
    /// it.
    private var pathObserver: PacketTunnelPathObserver?

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
                    self.startPathObservation()
                    Task { await self.implementation.startTunnel(options: startOptions) }
                }
                self.internalQueue.async {
                    completionHandler(error)
                }
            })
    }

    override func stopTunnel(with reason: NEProviderStopReason) async {
        providerLogger.debug("stopTunnel: \(ProviderStopReasonWrapper(reason: reason))")

        pathObserver?.stop()
        pathObserver = nil
        await implementation.stopTunnel()
    }

    /// Start (or restart) NWPath observation and forward each debounced
    /// verdict to the actor. Idempotent per tunnel session.
    private func startPathObservation() {
        pathObserver?.stop()
        let observer = PacketTunnelPathObserver(eventQueue: internalQueue)
        pathObserver = observer
        observer.start { [weak self] status in
            self?.implementation.actor.updateNetworkReachability(networkPathStatus: status)
        }
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
        tunnelNetworkSettings(
            ipv4Address: LocalNetworkIPs.gatewayAddressIpV4.rawValue,
            ipv4SubnetMask: "255.255.255.255"
        )
    }

    /// Builds the tunnel network settings with the given IPv4 interface
    /// address. The bootstrap call uses the placeholder
    /// `LocalNetworkIPs.gatewayAddressIpV4`; the multi-hop reassign path
    /// (`reapplyWarrenTunnelIPv4`) rebuilds with the exit-allocated
    /// address so only the IPv4 interface address changes, leaving DNS,
    /// MTU, the IPv6 blackhole, and the default route identical.
    private func tunnelNetworkSettings(
        ipv4Address: String,
        ipv4SubnetMask: String
    ) -> NEPacketTunnelNetworkSettings {
        let settings = NEPacketTunnelNetworkSettings(
            tunnelRemoteAddress: "\(IPv4Address.loopback)"
        )

        // IPv4 settings
        let ipv4Settings = NEIPv4Settings(
            addresses: [ipv4Address],
            subnetMasks: [ipv4SubnetMask]
        )
        ipv4Settings.includedRoutes = [NEIPv4Route.default()]
        settings.ipv4Settings = ipv4Settings

        // IPv6 blackhole (privacy invariant, aligned with Android's
        // `planTunInterface` and the desktop `enable_ipv6 = false` default).
        // The ULA address + `::/0` included route capture ALL IPv6 into the
        // tunnel so it can never leak to the physical network. Warren does
        // not carry IPv6 yet, so captured v6 is dropped at the exit and apps
        // fall back to IPv4 (the ULA is deprioritized by RFC 6724 address
        // selection). On iOS a route cannot exist without an interface
        // address (unlike the Linux/Android tun), so assigning `fc00::1` is
        // the only way to install the `::/0` capture: do NOT remove the
        // address thinking it tightens the blackhole, it would instead drop
        // the v6 capture and reopen the leak. The blocked/error state keeps
        // these settings (WarrenQuinnActor does not reconfigure), so the
        // kill switch holds for IPv6 too.
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
        //
        // "Allow external DNS resolvers" (iOS port of the desktop toggle
        // that lifts the firewall DNS block): leave `matchDomains` nil so
        // these DNS settings apply as the ordinary primary-interface
        // resolver (the tunnel owns the default route) WITHOUT the
        // match-everything claim. NE semantics: the `[""]` sentinel is
        // what forces every query, including ones scoped to other
        // interfaces or issued by resolvers configured outside the
        // tunnel, onto the tunnel resolver; with `matchDomains = nil`
        // the tunnel resolver stays the system default but no longer
        // monopolizes, so externally configured resolvers keep working.
        // Only DNS monopolization is lifted: the default route still
        // captures the query packets, so they travel inside the tunnel
        // and the chosen resolver sees them (reduced privacy, default
        // OFF). The flag is honored wherever these settings are built:
        // tunnel start and the exit-IPv4 reapply; a toggle mid-session
        // reconnects via TunnelSettingsStrategy like every settings diff.
        let tunnelSettings = (try? SettingsReader().read().tunnelSettings) ?? LatestTunnelSettings()
        let dnsSettings = NEDNSSettings(servers: warrenDNSResolvers(from: tunnelSettings))
        if !tunnelSettings.allowExternalDns.isEnabled {
            dnsSettings.matchDomains = [""]
        }
        settings.dnsSettings = dnsSettings

        return settings
    }

    /// Resolves the DNS servers to advertise, honoring the user's DNS
    /// settings (custom / content-blocking) the same way desktop and
    /// Android do, and defaulting to the Warren exit DNS forwarder (the
    /// iOS analog of Android's `EXIT_DNS_RESOLVER`).
    private func warrenDNSResolvers(from tunnelSettings: LatestTunnelSettings) -> [String] {
        // The Warren exit DNS forwarder, used unless the user picked
        // custom resolvers or enabled content blocking.
        let warrenExitDNSResolver = "10.66.0.1"

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

// Warren identifies users by wallet pubkey (allowlisted exit-side), not by a
// Mullvad account or registered device, so the legacy device-check subsystem
// is gone. Desktop and Android are already aligned.

// MARK: - Exit-allocated IP reassign

extension PacketTunnelProvider: WarrenTunnelIPReassigning {
    /// Re-applies the tunnel settings with the exit-allocated IPv4. The
    /// Warren multi-hop exit hands each wallet a sticky IPv4 over the
    /// setup stream; until it is applied the TUN keeps the bootstrap
    /// placeholder and the exit drops return traffic (the
    /// "connected but no bytes" bug). Re-applying `NEPacketTunnelNetwork`
    /// settings is supported mid-session and leaves `packetFlow` valid.
    func reapplyWarrenTunnelIPv4(address: String, prefixLength: Int) async {
        let mask = Self.ipv4SubnetMask(prefixLength: prefixLength)
        let settings = tunnelNetworkSettings(ipv4Address: address, ipv4SubnetMask: mask)
        await withCheckedContinuation { (continuation: CheckedContinuation<Void, Never>) in
            setTunnelNetworkSettings(settings) { error in
                if let error {
                    self.providerLogger.error(
                        "Failed to re-apply tunnel settings for exit IP \(address)/\(prefixLength): \(error)"
                    )
                } else {
                    self.providerLogger.debug(
                        "Re-applied tunnel network settings with exit-allocated IP \(address)/\(prefixLength)"
                    )
                }
                continuation.resume()
            }
        }
    }

    /// Convert an IPv4 prefix length (0-32) to a dotted-decimal subnet
    /// mask. Clamps out-of-range inputs to /32 (host route), the safe
    /// default the bootstrap settings already use.
    private static func ipv4SubnetMask(prefixLength: Int) -> String {
        let bits = (0...32).contains(prefixLength) ? prefixLength : 32
        let mask: UInt32 = bits == 0 ? 0 : ~UInt32(0) << (32 - bits)
        return "\((mask >> 24) & 0xFF).\((mask >> 16) & 0xFF).\((mask >> 8) & 0xFF).\(mask & 0xFF)"
    }
}
