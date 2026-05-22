//
//  SettingsViewControllerFactory.swift
//  MullvadVPN
//
//  Created by Jon Petersson on 2024-11-26.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import WarrenSettings
import Routing
import SwiftUI
import UIKit

@MainActor
final class SettingsViewControllerFactory {
    /// The result of creating a child representing a route.
    enum MakeChildResult {
        /// View controller that should be pushed into navigation stack.
        case viewController(UIViewController)

        /// Child coordinator that should be added to the children hierarchy.
        /// The child is responsile for presenting itself.
        case childCoordinator(SettingsChildCoordinator)

        /// Failure to produce a child.
        case failed
    }

    private let interactorFactory: SettingsInteractorFactory
    private let accessMethodRepository: AccessMethodRepositoryProtocol
    private let proxyConfigurationTester: ProxyConfigurationTesterProtocol
    private let breadcrumbsProvider: BreadcrumbsProvider
    private let ipOverrideRepository: IPOverrideRepository

    private let navigationController: UINavigationController
    private let alertPresenter: AlertPresenter
    private var appPreferences: AppPreferencesDataSource

    var didUpdateNotificationSettings: ((NotificationSettings) -> Void)?

    init(
        interactorFactory: SettingsInteractorFactory,
        accessMethodRepository: AccessMethodRepositoryProtocol,
        proxyConfigurationTester: ProxyConfigurationTesterProtocol,
        breadcrumbsProvider: BreadcrumbsProvider,
        ipOverrideRepository: IPOverrideRepository,
        navigationController: UINavigationController,
        alertPresenter: AlertPresenter,
        appPreferences: AppPreferencesDataSource
    ) {
        self.interactorFactory = interactorFactory
        self.accessMethodRepository = accessMethodRepository
        self.proxyConfigurationTester = proxyConfigurationTester
        self.breadcrumbsProvider = breadcrumbsProvider
        self.ipOverrideRepository = ipOverrideRepository
        self.navigationController = navigationController
        self.alertPresenter = alertPresenter
        self.appPreferences = appPreferences
    }

    func makeRoute(for route: SettingsNavigationRoute) -> MakeChildResult {
        switch route {
        case .root:
            // Handled in SettingsCoordinator.
            .failed
        case .faq:
            // Handled separately and presented as a modal.
            .failed
        case .language:
            // Handled separately and presented settings.
            .failed
        case .vpnSettings:
            makeVPNSettingsViewCoordinator()
        case .problemReport:
            makeProblemReportViewController()
        case .apiAccess:
            makeAPIAccessCoordinator()
        case .changelog:
            makeChangelogCoordinator()
        case .multihop:
            makeMultihopCoordinator()
        case .daita:
            makeDAITASettingsCoordinator()
        case .notificationSettings:
            makeNotificationSettingsCoordinator()
        case .includeAllNetworks:
            makeIncludeAllNetworksSettingsCoordinator()
        case .warrenWalletBackup:
            makeWarrenWalletBackupViewController()
        case .warrenWalletErase:
            makeWarrenWalletEraseViewController()
        case .warrenWalletIdentity:
            makeWarrenWalletIdentityViewController()
        case .warrenTunnelStatistics:
            makeWarrenTunnelStatisticsViewController()
        case .warrenDiagnosticInfo:
            makeWarrenDiagnosticInfoViewController()
        case .warrenAbout:
            makeWarrenAboutViewController()
        case .warrenPortForwarding:
            makeWarrenPortForwardingViewController()
        }
    }

    private func makeVPNSettingsViewCoordinator() -> MakeChildResult {
        return .childCoordinator(
            VPNSettingsCoordinator(
                navigationController: navigationController,
                interactorFactory: interactorFactory,
                ipOverrideRepository: ipOverrideRepository,
                route: .settings(.vpnSettings)
            ))
    }

    private func makeProblemReportViewController() -> MakeChildResult {
        return .viewController(
            ProblemReportViewController(
                interactor: interactorFactory.makeProblemReportInteractor(),
                alertPresenter: alertPresenter
            ))
    }

    private func makeAPIAccessCoordinator() -> MakeChildResult {
        return .childCoordinator(
            ListAccessMethodCoordinator(
                navigationController: navigationController,
                accessMethodRepository: accessMethodRepository,
                proxyConfigurationTester: proxyConfigurationTester,
                breadcrumbsProvider: breadcrumbsProvider,
                route: .settings(.apiAccess)
            ))
    }

    private func makeChangelogCoordinator() -> MakeChildResult {
        return .childCoordinator(
            ChangeLogCoordinator(
                route: .settings(.changelog),
                navigationController: navigationController,
                viewModel: ChangeLogViewModel(changeLogReader: ChangeLogReader())
            )
        )
    }

    private func makeMultihopCoordinator() -> MakeChildResult {
        let viewModel = MultihopTunnelSettingsViewModel(tunnelManager: interactorFactory.tunnelManager)
        let coordinator = MultihopSettingsCoordinator(
            navigationController: navigationController,
            route: .settings(.multihop),
            viewModel: viewModel
        )

        return .childCoordinator(coordinator)
    }

    private func makeDAITASettingsCoordinator() -> MakeChildResult {
        let viewModel = DAITATunnelSettingsViewModel(tunnelManager: interactorFactory.tunnelManager)
        let coordinator = DAITASettingsCoordinator(
            navigationController: navigationController,
            route: .settings(.daita),
            viewModel: viewModel
        )

        return .childCoordinator(coordinator)
    }

    private func makeNotificationSettingsCoordinator() -> MakeChildResult {
        let coordinator = NotificationSettingsCoordinator(
            navigationController: navigationController,
            viewModel: NotificationSettingsViewModel(settings: appPreferences.notificationSettings)
        )
        coordinator.didUpdateNotificationSettings = { [weak self] _, newValue in
            guard let self else { return }
            appPreferences.notificationSettings = newValue
            didUpdateNotificationSettings?(newValue)
        }

        return .childCoordinator(coordinator)
    }

    private func makeIncludeAllNetworksSettingsCoordinator() -> MakeChildResult {
        let viewModel = IncludeAllNetworksSettingsViewModelImpl(
            tunnelManager: interactorFactory.tunnelManager,
            appPreferences: appPreferences
        )
        let coordinator = IncludeAllNetworksSettingsCoordinator(
            navigationController: navigationController,
            route: .settings(.includeAllNetworks),
            viewModel: viewModel
        )

        return .childCoordinator(coordinator)
    }

    // MARK: - Warren-specific routes

    /// Wallet backup view (Settings → Recovery phrase, Face ID gated).
    private func makeWarrenWalletBackupViewController() -> MakeChildResult {
        let interactor = WarrenWalletInteractor()
        let controller = WarrenWalletBackupViewController(interactor: interactor)
        return .viewController(controller)
    }

    /// Wallet wipe (Settings → Erase wallet, destructive). Pushes the
    /// dedicated VC with its own confirmation alert.
    private func makeWarrenWalletEraseViewController() -> MakeChildResult {
        let controller = WarrenWalletEraseViewController()
        return .viewController(controller)
    }

    /// Wallet identity (Settings → Wallet identity, read-only pubkey).
    /// Loads the mnemonic from the Keychain synchronously (the iOS
    /// Keychain entry uses `kSecAttrAccessibleWhenUnlockedThisDeviceOnly`
    /// so the device must be unlocked, but no extra Face ID prompt is
    /// required since the pubkey is non-secret — sharing it cannot
    /// grant wallet access).
    private func makeWarrenWalletIdentityViewController() -> MakeChildResult {
        // `publicKeyHex()` returns nil when no wallet is present ; the
        // row is hidden in that case (see `SettingsDataSource`), so
        // this is defensive only — fall back to an empty hex string
        // which the view renders as a blank field.
        let hex = WarrenWalletInteractor().publicKeyHex() ?? ""
        let view = WarrenWalletIdentityView(pubkeyHex: hex)
        let host = UIHostingController(rootView: view)
        host.view.backgroundColor = .Warren.navy
        host.title = String(localized: "Wallet identity", table: "Wallet")
        return .viewController(host)
    }

    /// Tunnel statistics (Settings → Tunnel statistics, read-only).
    /// Snapshot built from App Group `UserDefaults` written by the
    /// PacketTunnel extension. Refreshes on each push (no continuous
    /// poll).
    private func makeWarrenTunnelStatisticsViewController() -> MakeChildResult {
        // Live source : the view's TimelineView re-fetches every 2 s,
        // matching the PacketTunnel extension's stats broadcast cadence.
        let view = WarrenTunnelStatisticsView(fetch: Self.loadTunnelStatistics)
        let host = UIHostingController(rootView: view)
        host.view.backgroundColor = .Warren.navy
        host.title = String(localized: "Tunnel statistics", table: "Settings")
        return .viewController(host)
    }

    /// Read tunnel session counters from App Group UserDefaults. The
    /// counters are populated by `WarrenQuinnAdapter.status()` snapshots
    /// broadcast from the PacketTunnel extension. Returns a
    /// disconnected/zero snapshot when no live tunnel exists.
    static func loadTunnelStatistics() -> WarrenTunnelStatistics {
        let suite = Bundle.main.object(forInfoDictionaryKey: "ApplicationSecurityGroupIdentifier") as? String
        let defaults = suite.flatMap { UserDefaults(suiteName: $0) }
        let bytesIn = UInt64(defaults?.integer(forKey: WarrenAppGroupKey.bytesIn.rawValue) ?? 0)
        let bytesOut = UInt64(defaults?.integer(forKey: WarrenAppGroupKey.bytesOut.rawValue) ?? 0)
        let duration = UInt64(defaults?.integer(forKey: WarrenAppGroupKey.connectedDurationSeconds.rawValue) ?? 0)
        let failover = UInt32(defaults?.integer(forKey: WarrenAppGroupKey.failoverCount.rawValue) ?? 0)
        let stateLabel = defaults?.string(forKey: WarrenAppGroupKey.stateLabel.rawValue)
            ?? String(localized: "Disconnected", table: "Settings", comment: "Default tunnel state when no live session")
        return WarrenTunnelStatistics(
            stateLabel: stateLabel,
            bytesIn: bytesIn,
            bytesOut: bytesOut,
            connectedDurationSeconds: duration > 0 ? duration : nil,
            failoverCount: failover
        )
    }

    /// Diagnostic info (Settings → Diagnostic info, screenshot-safe
    /// support payload). Builds the snapshot from bundle version +
    /// optional wallet pubkey (when wallet present) + App Group tunnel
    /// stats.
    private func makeWarrenDiagnosticInfoViewController() -> MakeChildResult {
        // Reuse the Mullvad-pattern `Bundle.shortVersion` / build
        // accessors so the same strings surface here as in
        // `Bundle.main.productVersion` (used by ConsolidatedApplicationLog
        // + Changelog).
        let appVersion = Bundle.main.shortVersion
        let build = Bundle.main.object(forInfoDictionaryKey: kCFBundleVersionKey as String) as? String ?? "?"
        // `<first 4 hex>...<last 4 hex>` short form fits one line on
        // small phones but still disambiguates between wallets.
        let walletShort = WarrenWalletInteractor().publicKeyShort()
        let info = WarrenDiagnosticInfo(
            appVersion: appVersion,
            buildNumber: build,
            walletPubkeyShortHex: walletShort,
            tunnelStats: Self.loadTunnelStatistics()
        )
        let view = WarrenDiagnosticInfoView(info: info)
        let host = UIHostingController(rootView: view)
        host.view.backgroundColor = .Warren.navy
        host.title = String(localized: "Diagnostic info", table: "Settings")
        return .viewController(host)
    }

    /// About Warren (Settings → About). Marketing + privacy + ToS
    /// + AGPL source code links + version banner.
    private func makeWarrenAboutViewController() -> MakeChildResult {
        let appVersion = Bundle.main.shortVersion
        let build = Bundle.main.object(forInfoDictionaryKey: kCFBundleVersionKey as String) as? String ?? "?"
        let view = WarrenAboutView(appVersion: appVersion, buildNumber: build)
        let host = UIHostingController(rootView: view)
        host.view.backgroundColor = .Warren.navy
        host.title = String(localized: "About Warren", table: "Settings")
        return .viewController(host)
    }

    /// NAT-PMP port forwarding settings (Settings → Port forwarding).
    private func makeWarrenPortForwardingViewController() -> MakeChildResult {
        let view = WarrenNatPmpSettingsView()
        let controller = UIHostingController(rootView: view)
        controller.view.backgroundColor = .Warren.navy
        controller.title = String(localized: "Port forwarding", table: "Settings")
        return .viewController(controller)
    }
}
