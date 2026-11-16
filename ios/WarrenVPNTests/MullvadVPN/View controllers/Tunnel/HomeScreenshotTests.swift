//
//  HomeScreenshotTests.swift
//  WarrenVPNTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import SwiftUI
import UIKit
import WarrenMockData
import WarrenREST
import WarrenRustRuntime
import WarrenSettings
import WarrenTypes
import XCTest

@testable import WarrenVPN

/// Renders the home under its real floating header, phase by phase, country by
/// country, in both themes, on whatever simulator runs it, to check by eye that
/// no landmark is cut or hidden under the toolbar or the fade. Opt-in: runs only
/// with `TEST_RUNNER_WARREN_QA_SHOT_DIR=<dir>` given to `xcodebuild test`.
@MainActor
final class HomeScreenshotTests: XCTestCase {
    private var directory: URL!
    private var landscape = false
    /// A window size to lay the home out at instead of the screen's.
    private var size: CGSize?

    override func setUpWithError() throws {
        guard let path = ProcessInfo.processInfo.environment["WARREN_QA_SHOT_DIR"] else {
            throw XCTSkip("set TEST_RUNNER_WARREN_QA_SHOT_DIR to capture the home")
        }
        directory = URL(fileURLWithPath: path, isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    }

    private struct Shot {
        let name: String
        let state: TunnelState
        var exitLocations: RelayConstraint<UserSelectedRelays> = .any
        var chips = false
    }

    private static func relays(_ country: String, _ code: String, _ city: String, entry: Bool = false) -> SelectedRelays
    {
        SelectedRelays(
            entry: entry
                ? HomeStatusTests.relay(hostname: "warren-0a1b2c3d", country: "Sweden", code: "se", city: "Stockholm")
                : nil,
            exit: HomeStatusTests.relay(hostname: "warren-3cb774609e5b6876", country: country, code: code, city: city),
            retryAttempt: 0
        )
    }

    private static func connected(_ country: String, _ code: String, _ city: String) -> TunnelState {
        .connected(relays(country, code, city), isPostQuantum: false, isDaita: false)
    }

    private static let shots: [Shot] = [
        Shot(name: "01-exposed", state: .disconnected),
        Shot(
            name: "02-exposed-picked-sweden", state: .disconnected,
            exitLocations: .only(UserSelectedRelays(locations: [.country("se")]))),
        Shot(
            name: "03-connecting-netherlands",
            state: .connecting(relays("Netherlands", "nl", "Amsterdam"), isPostQuantum: false, isDaita: false)),
        Shot(name: "04-protected-netherlands", state: connected("Netherlands", "nl", "Amsterdam"), chips: true),
        Shot(name: "05-protected-france", state: connected("France", "fr", "Paris"), chips: true),
        Shot(name: "06-protected-finland", state: connected("Finland", "fi", "Helsinki")),
        Shot(name: "07-protected-singapore", state: connected("Singapore", "sg", "Singapore")),
        Shot(name: "08-protected-germany", state: connected("Germany", "de", "Berlin")),
        Shot(name: "09-protected-romania", state: connected("Romania", "ro", "Bucharest")),
        Shot(
            name: "10-protected-sweden-two-hops",
            state: .connected(relays("Sweden", "se", "Gothenburg", entry: true), isPostQuantum: false, isDaita: false)),
        Shot(name: "11-interrupted-finland", state: .waitingForConnectivity(.noNetwork)),
        Shot(name: "12-blocked", state: .waitingForConnectivity(.noConnection)),
    ]

    func testCaptureTheHome() async throws {
        let device = UIDevice.current.userInterfaceIdiom == .pad ? "ipad" : "iphone"
        for theme in [WarrenTheme.dark, .light] {
            for shot in Self.shots {
                let host = makeHost(shot: shot, theme: theme, badge: theme == .light ? "BETA" : nil)
                let window = try await window(with: host)
                try await save(window, "\(device)-\(theme)-\(shot.name).png")
            }
        }
    }

    /// A tablet in landscape: the framing narrows and centres the canvas, and
    /// the panel is a centred column.
    func testCaptureTheHomeInLandscapeOnATablet() async throws {
        guard UIDevice.current.userInterfaceIdiom == .pad else {
            throw XCTSkip("landscape is a tablet layout")
        }
        // Turn the scene for real, so the safe area is the landscape one; where the request is
        // refused the window is laid out at the landscape size of the portrait screen instead.
        landscape = true
        if let scene = UIApplication.shared.connectedScenes.compactMap({ $0 as? UIWindowScene }).first {
            scene.requestGeometryUpdate(.iOS(interfaceOrientations: .landscapeRight))
            try await Task.sleep(for: .milliseconds(1500))
        }
        defer {
            UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first?
                .requestGeometryUpdate(.iOS(interfaceOrientations: .portrait))
        }
        for theme in [WarrenTheme.dark, .light] {
            for shot in Self.shots where ["01", "03", "04", "07", "10"].contains(String(shot.name.prefix(2))) {
                let window = try await window(with: makeHost(shot: shot, theme: theme, badge: nil))
                try await save(window, "ipad-landscape-\(theme)-\(shot.name).png")
            }
            let details = makeHost(
                shot: Shot(name: "details", state: Self.connected("Finland", "fi", "Helsinki"), chips: true),
                theme: theme, badge: nil)
            let detailsWindow = try await window(with: details)
            details.home.openDetails()
            try await save(detailsWindow, "ipad-landscape-\(theme)-13-details.png")
        }
    }

    /// The connection details open in place under the exit row, over a compact hero.
    func testCaptureTheDetails() async throws {
        let device = UIDevice.current.userInterfaceIdiom == .pad ? "ipad" : "iphone"
        for theme in [WarrenTheme.dark, .light] {
            let host = makeHost(
                shot: Shot(name: "details", state: Self.connected("Finland", "fi", "Helsinki"), chips: true),
                theme: theme,
                badge: nil
            )
            let window = try await window(with: host)
            host.home.openDetails()
            try await save(window, "\(device)-\(theme)-13-details.png")
        }
    }

    /// The mockup's states at the sizes a phone or a tablet can give the home: a small phone, a
    /// large one, a tall 20:9 one, two short windows wider than tall (split side by side) and a
    /// narrow split-screen column. The window is
    /// laid out at that size on whatever simulator runs it.
    func testCaptureTheMockupStatesAtEverySize() async throws {
        let sizes: [(String, CGSize)] = [
            ("small-360x640", CGSize(width: 360, height: 640)),
            ("phone-412x915", CGSize(width: 412, height: 915)),
            ("tall-360x800", CGSize(width: 360, height: 800)),
            ("split-844x390", CGSize(width: 844, height: 390)),
            ("split-700x500", CGSize(width: 700, height: 500)),
            ("narrow-320x700", CGSize(width: 320, height: 700)),
        ]
        let cap = WarrenNetworkInfo(
            environment: "beta", degraded: true, defaultRateBps: 20_000_000, paymentsEnabled: false)
        let update = NewAppVersionInAppNotificationProvider.descriptor(
            version: "1.4.0", includeAllNetworksIsEnabled: false, install: {})
        for (label, windowSize) in sizes {
            size = windowSize
            for theme in [WarrenTheme.dark, .light] {
                let finland = Self.connected("Finland", "fi", "Helsinki")
                let exposed = makeHost(
                    shot: Shot(
                        name: "exposed", state: .disconnected,
                        exitLocations: .only(UserSelectedRelays(locations: [.country("fi")]))),
                    theme: theme, badge: nil)
                try await save(try await window(with: exposed), "\(label)-\(theme)-0-exposed.png")

                let protected = makeHost(
                    shot: Shot(name: "protected", state: finland, chips: true), theme: theme, badge: nil)
                protected.setForumSlot(.activity)
                protected.setForumUnread(3)
                try await save(try await window(with: protected), "\(label)-\(theme)-1-protected.png")

                let details = makeHost(
                    shot: Shot(name: "details", state: finland, chips: true), theme: theme, badge: nil)
                let detailsWindow = try await window(with: details)
                details.home.openDetails()
                try await save(detailsWindow, "\(label)-\(theme)-2-details.png")

                let beta = makeHost(
                    shot: Shot(name: "beta", state: .disconnected), theme: theme, badge: "BETA", networkInfo: cap)
                let notifications = NotificationController()
                beta.notificationController = notifications
                let betaWindow = try await window(with: beta)
                notifications.setNotifications([update], animated: false)
                try await save(betaWindow, "\(label)-\(theme)-3-update-beta.png")
            }
        }
        size = nil
    }

    /// Mockup 8d on a beta build: the update banner over the exposed home, an announcement over
    /// Finland with its flag on the location row, and the cap in the footer.
    func testCaptureTheBannersAndTheBetaFooter() async throws {
        let device = UIDevice.current.userInterfaceIdiom == .pad ? "ipad" : "iphone"
        let cap = WarrenNetworkInfo(
            environment: "beta", degraded: true, defaultRateBps: 20_000_000, paymentsEnabled: false)
        let update = NewAppVersionInAppNotificationProvider.descriptor(
            version: "2026.5", includeAllNetworksIsEnabled: false, install: {})
        let announcements = WarrenAnnouncementNotificationProvider(
            source: {
                [
                    WarrenAnnouncement(
                        id: "qa", headline: "New servers in Singapore", body: "Available now", level: .info,
                        cta: nil, voucherCampaignID: nil, voucherCode: nil)
                ]
            },
            dismissStore: QADismissStore(),
            present: { _ in }
        )
        let announcement = try XCTUnwrap(announcements.notificationDescriptor)
        let shots: [(Shot, InAppNotificationDescriptor)] = [
            (Shot(name: "14-8d-exposed-update", state: .disconnected), update),
            (
                Shot(
                    name: "15-8d-exposed-picked-finland-update", state: .disconnected,
                    exitLocations: .only(UserSelectedRelays(locations: [.country("fi")]))),
                update
            ),
            (
                Shot(name: "16-8d-protected-finland-announcement", state: Self.connected("Finland", "fi", "Helsinki")),
                announcement
            ),
        ]
        for theme in [WarrenTheme.dark, .light] {
            for (shot, notification) in shots {
                let host = makeHost(shot: shot, theme: theme, badge: "BETA", networkInfo: cap)
                let notifications = NotificationController()
                host.notificationController = notifications
                let window = try await window(with: host)
                notifications.setNotifications([notification], animated: false)
                try await save(window, "\(device)-\(theme)-\(shot.name).png")
            }
        }
    }

    // MARK: - Rendering

    private func makeHost(
        shot: Shot,
        theme: WarrenTheme,
        badge: String?,
        networkInfo: WarrenNetworkInfo? = nil
    ) -> QARootContainer {
        let constraints = RelayConstraints(exitLocations: shot.exitLocations)
        let connection = ConnectionViewViewModel(
            tunnelStatus: TunnelStatus(state: shot.state),
            relayConstraints: constraints,
            relayCache: MockRelayCache(),
            customListRepository: CustomListRepository()
        )
        connection.productBadge = badge
        connection.networkInfo = networkInfo
        var settings = LatestTunnelSettings(relayConstraints: constraints)
        if shot.chips {
            settings.natPmp = WarrenNatPmpSettings(state: .on)
            settings.allowExternalDns = WarrenAllowExternalDnsSettings(state: .on)
        }
        let indicators = FeatureIndicatorsViewModel(
            tunnelSettings: settings,
            tunnelStatus: TunnelStatus(state: shot.state)
        )
        let footer = HomeFooterModel()
        footer.pubkey = "5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY"
        footer.expiry = Date().addingTimeInterval(29.5 * 86_400)

        let home = HomeViewController(
            connectionViewModel: connection,
            indicatorsViewModel: indicators,
            footerModel: footer
        )
        home.theme = theme
        let exitCountry =
            TunnelViewController.targetCountry(
                tunnelState: shot.state, exitLocations: shot.exitLocations, lastExitCountry: "Finland")
        home.update(phase: ConnectionPhase(tunnelState: shot.state), exitCountry: exitCountry, animated: false)

        let root = QARootContainer(home: home, theme: theme)
        root.setViewControllers([root.screen], animated: false)
        root.update(configuration: RootConfiguration(showsAccountButton: true))
        root.setForumSlot(.community)
        return root
    }

    private func window(with root: UIViewController) async throws -> UIWindow {
        let scene = try XCTUnwrap(
            UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first)
        let window = UIWindow(windowScene: scene)
        let bounds = scene.screen.bounds
        if let size {
            window.frame = CGRect(origin: .zero, size: size)
        } else {
            window.frame =
                landscape
                ? CGRect(
                    x: 0, y: 0, width: max(bounds.width, bounds.height), height: min(bounds.width, bounds.height))
                : bounds
        }
        window.rootViewController = root
        window.makeKeyAndVisible()
        try await Task.sleep(for: .milliseconds(900))
        return window
    }

    private func save(_ window: UIWindow, _ name: String) async throws {
        // The blurred landscape renders off the main thread; let it land.
        try await Task.sleep(for: .milliseconds(1200))
        let renderer = UIGraphicsImageRenderer(bounds: window.bounds)
        let image = renderer.image { _ in
            window.drawHierarchy(in: window.bounds, afterScreenUpdates: true)
        }
        try XCTUnwrap(image.pngData()).write(to: directory.appendingPathComponent(name))
        window.isHidden = true
    }
}

/// The root container with one screen in it that presents the home's header,
/// the way the tunnel screen does.
private final class QARootContainer: RootContainerViewController {
    let screen: QAHomeScreen

    var home: HomeViewController { screen.home }

    init(home: HomeViewController, theme: WarrenTheme) {
        screen = QAHomeScreen(home: home, theme: theme)
        super.init(nibName: nil, bundle: nil)
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }
}

private final class QAHomeScreen: UIViewController, RootContainment {
    let home: HomeViewController
    let theme: WarrenTheme

    init(home: HomeViewController, theme: WarrenTheme) {
        self.home = home
        self.theme = theme
        super.init(nibName: nil, bundle: nil)
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    var preferredHeaderBarPresentation: HeaderBarPresentation {
        HeaderBarPresentation(
            style: .transparent, showsDivider: false, tone: .dark, showsProductChip: false, home: theme)
    }

    var prefersHeaderBarHidden: Bool { false }
    var prefersDeviceInfoBarHidden: Bool { true }
    var prefersNotificationBarHidden: Bool { false }

    override func viewDidLoad() {
        super.viewDidLoad()
        addChild(home)
        view.addConstrainedSubviews([home.view]) {
            home.view.pinEdgesToSuperview()
        }
        home.didMove(toParent: self)
    }
}

private final class QADismissStore: WarrenAnnouncementDismissing {
    var warrenDismissedAnnouncements: [String] = []
}
