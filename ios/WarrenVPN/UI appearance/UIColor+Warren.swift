//
//  UIColor+Warren.swift
//  WarrenVPN
//
//  Created by Warren on 2026-05-21.
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  Warren-specific brand tokens that complement the upstream Mullvad
//  palette (`UIColor+Palette.swift`). Used by wallet/onboarding/Warren
//  feature views. Colors must stay in sync with the warrenbrowse.com
//  marketing site and the Electron desktop app.
//

import SwiftUI
import UIKit

extension UIColor {
    enum Warren {
        /// Brand ocre `#CA963C` (Bula art direction), same token as
        /// `.warningColor`. Used sparingly for primary actions + accent
        /// highlights only, to preserve its signal value.
        static let yellow = UIColor.warningColor

        /// Dark backdrop in wallet/onboarding flows: the neutral charcoal
        /// canvas `#1F1F20`, same token as `.secondaryColor`. Neutrals stay
        /// truly neutral; all warmth comes from the accents.
        static let navy = UIColor.secondaryColor

        /// Soft surface for cards on top of `.navy`. Slightly lighter
        /// neutral grey `#2A2928` to delineate without losing dark-mode feel.
        static let surface = UIColor(red: 42.0 / 255.0, green: 41.0 / 255.0, blue: 40.0 / 255.0, alpha: 1.0)

        /// Subtle ocre tint for borders / focus rings. Use at low
        /// alpha to avoid eyestrain.
        static let accentTint = yellow.withAlphaComponent(0.6)

        /// Critical/error state. Inherits from upstream `.dangerColor`
        /// so the palette stays single-source-of-truth.
        static let error = UIColor.dangerColor

        /// Success state. Inherits from upstream `.successColor`.
        static let success = UIColor.successColor
    }
}

extension Color {
    /// SwiftUI bridge to the UIKit Warren palette. Prefer this over raw
    /// RGB literals in SwiftUI views.
    enum Warren {
        static let yellow = Color(UIColor.Warren.yellow)
        static let navy = Color(UIColor.Warren.navy)
        static let surface = Color(UIColor.Warren.surface)
        static let accentTint = Color(UIColor.Warren.accentTint)
        static let error = Color(UIColor.Warren.error)
        static let success = Color(UIColor.Warren.success)
    }
}
