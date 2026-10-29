//
//  WarrenDeviceLaunchTests.swift
//  WarrenVPNTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import Foundation
import WarrenRustRuntime
import XCTest

@testable import WarrenVPN

/// The app process lists, renames and revokes the account's devices, and the
/// registry marks "This device" by the identity the call carries. The app has
/// to ask as the device the extension registers, which it only does once its
/// launch configured the registry on the extension's directory.
final class WarrenDeviceLaunchTests: XCTestCase {
    /// The tests run hosted in the app, so its launch already ran.
    func testTheAppLaunchConfiguresItsDeviceWhereTheExtensionKeepsIt() {
        let extensionPinStore = ApplicationConfiguration.containerURL
            .appendingPathComponent("warren-exit-pins.json").path

        XCTAssertEqual(
            WarrenDeviceLaunch.configuredDirectory,
            WarrenQuinnAdapter.deviceStateDirectory(pinStorePath: extensionPinStore)
        )
    }
}
