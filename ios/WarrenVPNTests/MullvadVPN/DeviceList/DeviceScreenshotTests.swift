//
//  DeviceScreenshotTests.swift
//  WarrenVPNTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import PacketTunnelCore
import SwiftUI
import UIKit
import WarrenRustRuntime
import XCTest

@testable import WarrenVPN

/// Renders the device list, its confirmation, its rename dialog and the two
/// banners that open it, from a registry answered in memory, to compare the
/// three clients by eye. Opt-in: runs only with
/// `TEST_RUNNER_WARREN_SCREENSHOTS_DIR=<dir>` given to `xcodebuild test`.
@MainActor
final class DeviceScreenshotTests: XCTestCase {
    private static let screen = CGRect(x: 0, y: 0, width: 402, height: 874)

    private var directory: URL!

    override func setUpWithError() throws {
        guard let path = ProcessInfo.processInfo.environment["WARREN_SCREENSHOTS_DIR"] else {
            throw XCTSkip("set TEST_RUNNER_WARREN_SCREENSHOTS_DIR to capture the device screens")
        }
        directory = URL(fileURLWithPath: path, isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    }

    func testCaptureTheDeviceList() async throws {
        let viewModel = DeviceManagementViewModel(deviceManaging: MockDeviceManaging())
        let window = try await show(DeviceManagementView(viewModel: viewModel))
        await viewModel.fetchDevices()
        try await save(window, "device-list.png")
    }

    func testCaptureTheListThatCouldNotLoad() async throws {
        let viewModel = DeviceManagementViewModel(deviceManaging: MockDeviceManaging(list: .failure(.unavailable)))
        let window = try await show(DeviceManagementView(viewModel: viewModel))
        await viewModel.fetchDevices()
        try await save(window, "device-list-error.png")
    }

    func testCaptureTheDisconnectConfirmation() async throws {
        let device = MockDeviceManaging.sample.devices[1]
        let alert = DeviceManagementView.disconnectConfirmation(for: device, onDisconnect: {}, onCancel: {})
        let window = try await show(Color.warrenBackground.warrenAlert(item: .constant(alert)))
        try await save(window, "disconnect-confirmation.png")
    }

    func testCaptureTheRenameDialog() async throws {
        let device = MockDeviceManaging.sample.devices[1]
        let alert = DeviceManagementView.renameAlert(for: device) { _ in }
        let window = try await show(Color.warrenBackground.warrenInputAlert(item: .constant(alert)))
        try await save(window, "rename.png")
    }

    func testCaptureTheDeviceLimitBanner() async throws {
        try await saveBanner(for: .tooManyDevices, "banner-device-limit.png")
    }

    func testCaptureTheNoSessionTokenBanner() async throws {
        try await saveBanner(for: .noSessionToken, "banner-no-session-token.png")
    }

    // MARK: - Rendering

    private func show(_ view: some View) async throws -> UIWindow {
        let host = UIHostingController(rootView: view)
        host.title = NSLocalizedString("Devices", comment: "")
        let navigation = UINavigationController(rootViewController: host)
        navigation.navigationBar.prefersLargeTitles = true
        navigation.overrideUserInterfaceStyle = .dark
        return try await window(with: navigation)
    }

    private func saveBanner(for reason: BlockedStateReason, _ name: String) async throws {
        let banner = NotificationBannerView()
        banner.style = .error
        banner.title = NSLocalizedString("BLOCKING INTERNET", comment: "")
        banner.body = TunnelStatusNotificationProvider.notificationBody(
            TunnelStatusNotificationProvider.bodyText(for: reason))

        let controller = UIViewController()
        controller.view.backgroundColor = .secondaryColor
        controller.view.addSubview(banner)
        banner.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            banner.topAnchor.constraint(equalTo: controller.view.safeAreaLayoutGuide.topAnchor),
            banner.leadingAnchor.constraint(equalTo: controller.view.leadingAnchor),
            banner.trailingAnchor.constraint(equalTo: controller.view.trailingAnchor),
        ])
        let window = try await window(with: controller)
        try await save(window, name)
    }

    private func window(with root: UIViewController) async throws -> UIWindow {
        let scene = try XCTUnwrap(
            UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first)
        let window = UIWindow(windowScene: scene)
        window.frame = Self.screen
        window.overrideUserInterfaceStyle = .dark
        window.rootViewController = root
        window.makeKeyAndVisible()
        try await Task.sleep(for: .milliseconds(800))
        return window
    }

    private func save(_ window: UIWindow, _ name: String) async throws {
        try await Task.sleep(for: .milliseconds(800))
        let renderer = UIGraphicsImageRenderer(bounds: window.bounds)
        let image = renderer.image { _ in
            window.drawHierarchy(in: window.bounds, afterScreenUpdates: true)
        }
        try XCTUnwrap(image.pngData()).write(to: directory.appendingPathComponent(name))
        window.isHidden = true
    }
}
