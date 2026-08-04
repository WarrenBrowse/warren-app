//
//  UIColor+Palette.swift
//  MullvadVPN
//
//  Created by pronebird on 20/03/2019.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import UIKit

extension UIColor {
    enum AccountTextField {
        enum NormalState {
            static let borderColor = secondaryColor
            static let textColor = primaryColor
            static let backgroundColor = UIColor.white
        }

        enum ErrorState {
            static let borderColor = dangerColor.withAlphaComponent(0.4)
            static let textColor = dangerColor
            static let backgroundColor = UIColor.white
        }

        enum AuthenticatingState {
            static let borderColor = secondaryColor
            static let textColor = primaryColor
            static let backgroundColor = UIColor.white.withAlphaComponent(0.4)
        }
    }

    enum TextField {
        static let placeholderTextColor = UIColor(red: 0.290, green: 0.282, blue: 0.275, alpha: 0.40)
        static let inactivePlaceholderTextColor = UIColor(white: 1.0, alpha: 0.4)
        static let textColor = UIColor(red: 0.290, green: 0.282, blue: 0.275, alpha: 1.0)
        static let inactiveTextColor = UIColor.white
        static let backgroundColor = UIColor.white
        static let inactiveBackgroundColor = UIColor(white: 1.0, alpha: 0.1)
        static let invalidInputTextColor = UIColor.dangerColor
    }

    enum SearchTextField {
        static let placeholderTextColor = TextField.placeholderTextColor
        static let inactivePlaceholderTextColor = TextField.inactivePlaceholderTextColor
        static let textColor = TextField.textColor
        static let inactiveTextColor = TextField.inactiveTextColor
        static let backgroundColor = TextField.backgroundColor
        static let inactiveBackgroundColor = TextField.inactiveBackgroundColor
        static let leftViewTintColor = UIColor.primaryColor
        static let inactiveLeftViewTintColor = UIColor.white
    }

    enum AppButton {
        static let normalTitleColor = UIColor.white
        static let highlightedTitleColor = UIColor.lightGray
        static let disabledTitleColor = UIColor.white.withAlphaComponent(0.2)
    }

    enum Switch {
        static let borderColor = UIColor(white: 1.0, alpha: 0.8)
        static let onThumbColor = successColor
        static let offThumbColor = dangerColor
    }

    // Relay availability indicator view
    enum RelayStatusIndicator {
        static let activeColor = successColor.withAlphaComponent(0.9)
        static let inactiveColor = dangerColor.withAlphaComponent(0.95)
        static let highlightColor = UIColor.white
    }

    enum MainSplitView {
        static let dividerColor = UIColor.black
    }

    // Navigation bars
    enum NavigationBar {
        static let buttonColor = UIColor(white: 1.0, alpha: 0.8)
        static let backButtonTitleColor = UIColor.white
        static let titleColor = UIColor.white
        static let promptColor = UIColor.white
    }

    // Heading displayed below the navigation bar.
    enum ContentHeading {
        static let textColor = UIColor(white: 1.0, alpha: 0.6)
        static let linkColor = UIColor.white
    }

    // Cells
    enum Cell {
        enum Background {
            // Neutral grey surface ladder (desktop blue60/40/20/10).
            static let indentationLevelZero = UIColor(red: 0.282, green: 0.275, blue: 0.267, alpha: 1.0)
            static let indentationLevelOne = UIColor(red: 0.220, green: 0.216, blue: 0.208, alpha: 1.0)
            static let indentationLevelTwo = UIColor(red: 0.188, green: 0.184, blue: 0.180, alpha: 1.0)
            static let indentationLevelThree = UIColor(red: 0.165, green: 0.161, blue: 0.157, alpha: 1.0)

            static let normal = UIColor.primaryColor
            static let disabled = normal.darkened(by: 0.1)!
            static let selected = successColor
            static let disabledSelected = selected.darkened(by: 0.3)!
            static let selectedAlt = normal.darkened(by: 0.1)!
        }

        static let titleTextColor = UIColor.white
        static let detailTextColor = UIColor(white: 1.0, alpha: 0.8)

        static let disclosureIndicatorColor = UIColor(white: 1.0, alpha: 0.8)
        static let textFieldTextColor = UIColor.white
        static let textFieldPlaceholderColor = UIColor(white: 1.0, alpha: 0.6)

        static let validationErrorBorderColor = UIColor.dangerColor
    }

    enum TableSection {
        static let headerTextColor = UIColor(white: 1.0, alpha: 0.6)
        static let footerTextColor = UIColor(white: 1.0, alpha: 0.6)
    }

    enum SettingsCellBackground {}

    enum HeaderBar {
        static let defaultBackgroundColor = primaryColor
        static let unsecuredBackgroundColor = dangerColor
        static let securedBackgroundColor = successColor
        // Connecting/reconnecting: distinct pending orange (desktop tri-state).
        static let securingBackgroundColor = pendingColor
        static let dividerColor = secondaryColor
        static let brandNameColor = UIColor(white: 1.0, alpha: 0.8)
        static let buttonColor = UIColor(white: 1.0, alpha: 0.8)
        static let disabledButtonColor = UIColor(white: 1.0, alpha: 0.5)
    }

    enum InAppNotificationBanner {
        static let errorIndicatorColor = dangerColor
        static let successIndicatorColor = successColor
        static let warningIndicatorColor = warningColor

        static let titleColor = UIColor.white
        static let bodyColor = UIColor(white: 1.0, alpha: 0.6)
        static let actionButtonColor = UIColor(white: 1.0, alpha: 0.8)
    }

    enum SegmentedControl {
        static let backgroundColor = UIColor(red: 0.282, green: 0.275, blue: 0.267, alpha: 1.0)
        static let selectedColor = successColor
    }

    // Map polygons and ocean fill, kept in lockstep with the desktop map
    // (land neutral grey, ocean near-black charcoal). Neutral on purpose:
    // the map is a large area and must never read as a colour wash.
    enum Map {
        static let landColor = UIColor(red: 57.0 / 255.0, green: 57.0 / 255.0, blue: 59.0 / 255.0, alpha: 1.0)
        static let oceanColor = UIColor(red: 28.0 / 255.0, green: 28.0 / 255.0, blue: 30.0 / 255.0, alpha: 1.0)
    }

    enum AlertController {
        static let tintColor = UIColor(red: 0.0, green: 0.59, blue: 1.0, alpha: 1)
    }

    // Common colors: Warren dark palette (Bula art direction), in lockstep
    // with the desktop source of truth (color-tokens.ts) and Android
    // PaletteTokens.kt. HARD RULE: the neutrals (primary/secondary surfaces)
    // are truly neutral grey and carry no hue; ALL warmth comes from the
    // accents only, otherwise the whole screen reads as a sepia wash.
    // Primary interactive surface (raised cells, buttons): neutral warm-grey #4A4846.
    static let primaryColor = UIColor(red: 74.0 / 255.0, green: 72.0 / 255.0, blue: 70.0 / 255.0, alpha: 1.0)
    // Main app background: neutral charcoal #1F1F20.
    static let secondaryColor = UIColor(red: 31.0 / 255.0, green: 31.0 / 255.0, blue: 32.0 / 255.0, alpha: 1.0)
    // Disconnected / error: terracotta #CA4C38.
    static let dangerColor = UIColor(red: 202.0 / 255.0, green: 76.0 / 255.0, blue: 56.0 / 255.0, alpha: 1.0)
    // Warning + brand accent: ocre #CA963C.
    static let warningColor = UIColor(red: 202.0 / 255.0, green: 150.0 / 255.0, blue: 60.0 / 255.0, alpha: 1.0)
    // Connected / success: olive-green #6EA24E.
    static let successColor = UIColor(red: 110.0 / 255.0, green: 162.0 / 255.0, blue: 78.0 / 255.0, alpha: 1.0)
    // Connecting / in-progress: a distinct orange between the exposed red and
    // the secured green, so the transitional state reads as its own phase.
    static let pendingColor = UIColor(red: 224.0 / 255.0, green: 122.0 / 255.0, blue: 40.0 / 255.0, alpha: 1.0)

    // True near-white #F7F7F8 (neutral, NOT cream).
    static let primaryTextColor = UIColor(red: 247.0 / 255.0, green: 247.0 / 255.0, blue: 248.0 / 255.0, alpha: 1.0)
    static let secondaryTextColor = UIColor(white: 1.0, alpha: 0.8)
}
