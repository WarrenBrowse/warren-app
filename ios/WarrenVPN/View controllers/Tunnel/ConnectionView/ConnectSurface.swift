//
//  ConnectSurface.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  The connect screen's own surfaces: the home page, its status, controls,
//  chips, toolbar and footer (the `home*` tokens), and the surfaces of the
//  card design they replaced that other views still draw with. They follow the
//  theme, light or dark, where the scenery accents of `SceneryTone` do not.
//
//  Every value here is a copy of `design-tokens.json` at the repository root
//  (sections `surfaces` and `components`), which desktop generates. iOS has no
//  generator, so `ConnectSurfaceTests` reads that file and fails on any drift.
//

import SwiftUI
import UIKit

/// The names of `design-tokens.json` `surfaces.<theme>`, spelled the same.
enum ConnectSurfaceToken: String, CaseIterable {
    case pill
    case text
    case textSecondary
    case button
    case buttonLine
    case homePage
    case homeText
    case homeTextSecondary
    case homeTextMono
    case homeField
    case homeFieldHover
    case homeFieldPressed
    case homeFieldIcon
    case homeFooter
    case homeChipText
    case homeChipLine
    case homeToolbar
    case homeToolbarLine
    case homeToolbarShadow
    case homeBadge
    case homeBadgeRing
    case homeScrim
    case homeLogoGlow
    case homePillExposed
    case homePillExposedText
    case homePillProtected
    case homePillProtectedText
    case homePillPending
    case homePillPendingText
    case homePillBlocked
    case homePillBlockedText
    case homeConnect
    case homeConnectHover
    case homeConnectPressed
    case homeConnectText
    case homeDisconnectLine
    case homeDisconnectText
    case homeDisconnectHover
    case homeCancelLine
    case homeCancelText
    case homeCancelHover
    case homeBeta
}

struct ConnectSurfacePalette: Equatable {
    let theme: WarrenTheme

    /// The token as `0xAARRGGBB`, the notation of `design-tokens.json`.
    func argb(_ token: ConnectSurfaceToken) -> UInt32 {
        switch theme {
        case .dark: Self.dark(token)
        case .light: Self.light(token)
        }
    }

    func uiColor(_ token: ConnectSurfaceToken) -> UIColor {
        let value = argb(token)
        return UIColor(
            red: CGFloat((value >> 16) & 0xFF) / 255,
            green: CGFloat((value >> 8) & 0xFF) / 255,
            blue: CGFloat(value & 0xFF) / 255,
            alpha: CGFloat((value >> 24) & 0xFF) / 255
        )
    }

    func color(_ token: ConnectSurfaceToken) -> Color {
        Color(uiColor: uiColor(token))
    }

    private static func dark(_ token: ConnectSurfaceToken) -> UInt32 {
        switch token {
        case .pill: 0xFFD9_A441
        case .text: 0xFFF2_EFE6
        case .textSecondary: 0xFFD6_D2C8
        case .button: 0xFF3A_3834
        case .buttonLine: 0xFF55_524B
        case .homePage: 0xFF14_1311
        case .homeText: 0xFFEF_E9DF
        case .homeTextSecondary: 0xFF9D_9488
        case .homeTextMono: 0xFF8A_8175
        case .homeField: 0xFF21_1F1C
        case .homeFieldHover: 0xFF2B_2824
        case .homeFieldPressed: 0xFF1C_1A18
        case .homeFieldIcon: 0xFFD6_CEC2
        case .homeFooter: 0xFF1B_1917
        case .homeChipText: 0xFFC3_BAAD
        case .homeChipLine: 0xFF36_322D
        case .homeToolbar: 0xB814_1311
        case .homeToolbarLine: 0x0FFF_FFFF
        case .homeToolbarShadow: 0x0000_0000
        case .homeBadge: 0xFFEF_B062
        case .homeBadgeRing: 0xFF1D_1B18
        case .homeScrim: 0x6614_1311
        case .homeLogoGlow: 0xB3FA_F7F0
        case .homePillExposed: 0xFFE9_8664
        case .homePillExposedText: 0xFF1C_0F0A
        case .homePillProtected: 0xFF7F_C581
        case .homePillProtectedText: 0xFF0D_160C
        case .homePillPending: 0xFFEF_B062
        case .homePillPendingText: 0xFF21_1404
        case .homePillBlocked: 0xFFD6_CEC2
        case .homePillBlockedText: 0xFF14_1311
        case .homeConnect: 0xFF7D_BD7F
        case .homeConnectHover: 0xFF8C_C98D
        case .homeConnectPressed: 0xFF6F_AC71
        case .homeConnectText: 0xFF0F_140E
        case .homeDisconnectLine: 0xFF92_4D35
        case .homeDisconnectText: 0xFFF6_A88D
        case .homeDisconnectHover: 0x1AE9_8664
        case .homeCancelLine: 0xFF93_6831
        case .homeCancelText: 0xFFF1_C17F
        case .homeCancelHover: 0x1AEF_B062
        case .homeBeta: 0xFFE8_B65C
        }
    }

    private static func light(_ token: ConnectSurfaceToken) -> UInt32 {
        switch token {
        case .pill: 0xFF7A_5412
        case .text: 0xFF2A_2822
        case .textSecondary: 0xFF4A_463D
        case .button: 0xFFEB_E3D0
        case .buttonLine: 0xFFCF_C2A5
        case .homePage: 0xFFF4_F0E8
        case .homeText: 0xFF22_1E19
        case .homeTextSecondary: 0xFF6F_675C
        case .homeTextMono: 0xFF7A_7266
        case .homeField: 0xFFE8_E2D6
        case .homeFieldHover: 0xFFDF_D8CA
        case .homeFieldPressed: 0xFFD6_CFC0
        case .homeFieldIcon: 0xFF3A_342C
        case .homeFooter: 0xFFEB_E5DA
        case .homeChipText: 0xFF5E_564B
        case .homeChipLine: 0xFFD6_CDBD
        case .homeToolbar: 0xCCFA_F7F0
        case .homeToolbarLine: 0x1428_1E14
        case .homeToolbarShadow: 0x4028_1E14
        case .homeBadge: 0xFFD9_7230
        case .homeBadgeRing: 0xFFFA_F7F0
        case .homeScrim: 0x59F4_F0E8
        case .homeLogoGlow: 0xB3FA_F7F0
        case .homePillExposed: 0xFFDF_7752
        case .homePillExposedText: 0xFF1C_0F0A
        case .homePillProtected: 0xFF67_B36A
        case .homePillProtectedText: 0xFF0D_160C
        case .homePillPending: 0xFFDF_9B44
        case .homePillPendingText: 0xFF21_1404
        case .homePillBlocked: 0xFF3A_342C
        case .homePillBlockedText: 0xFFF4_F0E8
        case .homeConnect: 0xFF3B_793F
        case .homeConnectHover: 0xFF44_8648
        case .homeConnectPressed: 0xFF32_6A36
        case .homeConnectText: 0xFFFF_FFFF
        case .homeDisconnectLine: 0xFFC2_6B4C
        case .homeDisconnectText: 0xFF9E_4421
        case .homeDisconnectHover: 0x1AC2_6B4C
        case .homeCancelLine: 0xFFBD_853F
        case .homeCancelText: 0xFF8D_5406
        case .homeCancelHover: 0x1ABD_853F
        case .homeBeta: 0xFF8A_5412
        }
    }
}

extension ConnectSurfacePalette {
    /// The palette of the colour scheme SwiftUI resolved for the view, which the
    /// hosting controller sets from the theme preference.
    init(colorScheme: ColorScheme) {
        self.init(theme: colorScheme == .light ? .light : .dark)
    }
}

extension ConnectSurfacePalette {
    /// The palette of the interface style a view resolved, dark unless it says
    /// light, the same rule as `WarrenTheme`.
    init(traitCollection: UITraitCollection) {
        self.init(theme: traitCollection.userInterfaceStyle == .light ? .light : .dark)
    }

    /// A token as a colour that follows whatever theme it is drawn in.
    static func dynamic(_ token: ConnectSurfaceToken) -> UIColor {
        UIColor { ConnectSurfacePalette(traitCollection: $0).uiColor(token) }
    }
}
