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
        /// Brand yellow `#FFD524` (cf. warrenbrowse.com landing CTAs,
        /// desktop accent, mascotte taupe accents). Used sparingly for
        /// primary actions + accent highlights only, to preserve its
        /// signal value.
        static let yellow = UIColor(red: 1.0, green: 213.0 / 255.0, blue: 36.0 / 255.0, alpha: 1.0)

        /// Brand navy `#0A1422` (cf. warrenbrowse.com background, desktop
        /// chrome). Used as the dark backdrop in wallet/onboarding
        /// flows so the brand yellow + white text pop.
        static let navy = UIColor(red: 10.0 / 255.0, green: 20.0 / 255.0, blue: 34.0 / 255.0, alpha: 1.0)

        /// Soft surface for cards on top of `.navy`. Slightly lighter
        /// to delineate without losing dark-mode feel.
        static let surface = UIColor(red: 18.0 / 255.0, green: 30.0 / 255.0, blue: 48.0 / 255.0, alpha: 1.0)

        /// Subtle yellow tint for borders / focus rings. Use at low
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
