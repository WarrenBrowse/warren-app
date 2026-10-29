//
//  WarrenDeviceLaunch.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import Foundation
import WarrenRustRuntime

/// Configures the app process's device in the account's registry
/// (warren-core doc 115). The extension configures its own process from the
/// App Group container; the app does the same at launch, so its list marks
/// this device and "Log out" revokes the identity the extension registered.
enum WarrenDeviceLaunch {
    /// The directory this process configured, `nil` before launch did.
    nonisolated(unsafe) private(set) static var configuredDirectory: String?

    /// Where the identity lives: the App Group container, the directory the
    /// extension derives from its pin table.
    static func stateDirectory(containerURL: URL) -> String {
        containerURL.path
    }

    static func configure(containerURL: URL) {
        let directory = stateDirectory(containerURL: containerURL)
        WarrenQuinnAdapter.configureDevice(stateDirectory: directory)
        configuredDirectory = directory
    }
}
