//
//  WarrenTheme.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import SwiftUI
import UIKit
import WarrenSettings

/// The palette a screen is actually painted in, resolved from the user's
/// choice and the device appearance. Desktop's `resolveTheme`.
enum WarrenTheme: Equatable, Sendable {
    case dark
    case light

    /// Dark is the house palette: a device that states no appearance paints
    /// dark, and so does an explicit dark choice on a light device.
    init(preference: WarrenThemePreference, systemStyle: UIUserInterfaceStyle) {
        switch preference {
        case .dark:
            self = .dark
        case .light:
            self = .light
        case .system:
            self = systemStyle == .light ? .light : .dark
        }
    }

    var userInterfaceStyle: UIUserInterfaceStyle {
        switch self {
        case .dark: .dark
        case .light: .light
        }
    }
}

extension WarrenThemePreference {
    /// The English source string of the choice, which is also its key in the
    /// Settings string table.
    var localizationKey: String {
        switch self {
        case .system: "System"
        case .dark: "Dark"
        case .light: "Light"
        }
    }

    var localizedTitle: String {
        NSLocalizedString(localizationKey, tableName: "Settings", comment: "")
    }

    /// The preference in force, read where the screens are built.
    static var current: WarrenThemePreference {
        AppPreferences().warrenThemePreference
    }

    /// Stores the choice and tells the screens that paint with it.
    func apply() {
        AppPreferences().warrenThemePreference = self
        NotificationCenter.default.post(name: .warrenThemePreferenceDidChange, object: nil)
    }
}

extension Notification.Name {
    static let warrenThemePreferenceDidChange = Notification.Name("WarrenThemePreferenceDidChange")
}
