//
//  ConnectSurface.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  The connect screen's own surfaces: the connection card, the feature chips
//  above it and the beta banner. They are opaque and follow the theme, light
//  or dark, where the scenery accents of `SceneryTone` do not.
//
//  Every value here is a copy of `design-tokens.json` at the repository root
//  (sections `surfaces` and `components`), which desktop generates. iOS has no
//  generator, so `ConnectSurfaceTests` reads that file and fails on any drift.
//

import SwiftUI
import UIKit

/// The names of `design-tokens.json` `surfaces.<theme>`, spelled the same.
enum ConnectSurfaceToken: String, CaseIterable {
    case card
    case line
    case shadowSoft
    case shadowStrong
    case pill
    case pillText
    case text
    case textSecondary
    case textMuted
    case button
    case buttonHover
    case buttonPressed
    case buttonLine
    case exposed
    case exposedWell
    case connecting
    case connectingWell
    case `protected`
    case protectedWell
    case connect
    case connectHover
    case connectPressed
    case disconnect
    case disconnectHover
    case disconnectPressed
    case cancel
    case cancelHover
    case cancelPressed
    case actionText
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
        case .card: 0xFF28_2623
        case .line: 0xFF45_423C
        case .shadowSoft: 0x5900_0000
        case .shadowStrong: 0x7300_0000
        case .pill: 0xFFD9_A441
        case .pillText: 0xFF23_1A06
        case .text: 0xFFF2_EFE6
        case .textSecondary: 0xFFD6_D2C8
        case .textMuted: 0xFFB5_B0A4
        case .button: 0xFF3A_3834
        case .buttonHover: 0xFF45_423D
        case .buttonPressed: 0xFF31_2F2B
        case .buttonLine: 0xFF55_524B
        case .exposed: 0xFFF0_8A6E
        case .exposedWell: 0xFF4A_2A22
        case .connecting: 0xFFF0_A360
        case .connectingWell: 0xFF4A_341C
        case .protected: 0xFF9F_D07E
        case .protectedWell: 0xFF24_3A1F
        case .connect: 0xFF3F_6B2E
        case .connectHover: 0xFF48_7A35
        case .connectPressed: 0xFF35_5A27
        case .disconnect: 0xFFA8_381F
        case .disconnectHover: 0xFFB8_4126
        case .disconnectPressed: 0xFF8F_2F1A
        case .cancel: 0xFFA0_5818
        case .cancelHover: 0xFFB0_621C
        case .cancelPressed: 0xFF8A_4C15
        case .actionText: 0xFFFF_FFFF
        }
    }

    private static func light(_ token: ConnectSurfaceToken) -> UInt32 {
        switch token {
        case .card: 0xFFF7_F1E3
        case .line: 0xFFD9_CDB2
        case .shadowSoft: 0x2E3C_321E
        case .shadowStrong: 0x473C_321E
        case .pill: 0xFF7A_5412
        case .pillText: 0xFFFF_FFFF
        case .text: 0xFF2A_2822
        case .textSecondary: 0xFF4A_463D
        case .textMuted: 0xFF5C_574C
        case .button: 0xFFEB_E3D0
        case .buttonHover: 0xFFE4_DAC3
        case .buttonPressed: 0xFFDC_D0B6
        case .buttonLine: 0xFFCF_C2A5
        case .exposed: 0xFFA3_321C
        case .exposedWell: 0xFFF3_D9CF
        case .connecting: 0xFF8A_4B0F
        case .connectingWell: 0xFFF3_E0C8
        case .protected: 0xFF2F_6A2A
        case .protectedWell: 0xFFD8_E8CC
        case .connect: 0xFF3F_6B2E
        case .connectHover: 0xFF48_7A35
        case .connectPressed: 0xFF35_5A27
        case .disconnect: 0xFFA8_381F
        case .disconnectHover: 0xFFB8_4126
        case .disconnectPressed: 0xFF8F_2F1A
        case .cancel: 0xFFA0_5818
        case .cancelHover: 0xFFB0_621C
        case .cancelPressed: 0xFF8A_4C15
        case .actionText: 0xFFFF_FFFF
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

/// `design-tokens.json` `components`, in points (dp and sp are points here).
enum ConnectSurfaceMetrics {
    enum Card {
        static let padding: CGFloat = 20
        static let radius: CGFloat = 16
        static let borderWidth: CGFloat = 0.5
        static let shadowOffsetY: CGFloat = 5
        static let shadowBlur: CGFloat = 16
        static let marginHorizontal: CGFloat = 14
        static let marginBottom: CGFloat = 6
        static let blockGap: CGFloat = 10.5
        static let chipGap: CGFloat = 2
        static let chipsToCardGap: CGFloat = 4
        static let chevronBoxSize: CGFloat = 22
        static let chevronIconSize: CGFloat = 18
    }

    enum Status {
        static let rowGap: CGFloat = 12
        static let textGap: CGFloat = 1
        static let trailingGap: CGFloat = 12
        static let wellSize: CGFloat = 34
        static let wellRadius: CGFloat = 8
        static let iconSize: CGFloat = 18
        static let titleSize: CGFloat = 16
        static let titleLineHeight: CGFloat = 19.2
        static let subtitleSize: CGFloat = 11.5
        static let subtitleLineHeight: CGFloat = 15
    }

    enum Location {
        static let gapAbove: CGFloat = 10.5
        static let size: CGFloat = 15
        static let lineHeight: CGFloat = 19
        static let hostnameGapAbove: CGFloat = 2
        static let hostnameSize: CGFloat = 12
        static let hostnameLineHeight: CGFloat = 16.5
    }

    enum Button {
        static let height: CGFloat = 32
        static let radius: CGFloat = 6
        static let borderWidth: CGFloat = 0.5
        static let textSize: CGFloat = 13
        static let textLineHeight: CGFloat = 18
        static let rowGap: CGFloat = 4
        static let shuffleWidth: CGFloat = 40
    }

    enum Chip {
        static let paddingVertical: CGFloat = 5.5
        static let paddingHorizontal: CGFloat = 8
        static let radius: CGFloat = 7
        static let borderWidth: CGFloat = 0.5
        static let shadowOffsetY: CGFloat = 1.5
        static let shadowBlur: CGFloat = 5
        /// Not in the shared table: desktop's chip text is 11px semibold.
        static let textSize: CGFloat = 11
    }

    enum Flag {
        static let size: CGFloat = 22
        static let ringWidth: CGFloat = 0.5
    }

    enum Banner {
        static let height: CGFloat = 36.5
        static let paddingStart: CGFloat = 8
        static let paddingEnd: CGFloat = 12
        static let gap: CGFloat = 8
        static let radius: CGFloat = 12
        static let borderWidth: CGFloat = 0.5
        static let shadowOffsetY: CGFloat = 2
        static let shadowBlur: CGFloat = 8
        static let pillHeight: CGFloat = 21
        static let pillPaddingHorizontal: CGFloat = 8
        static let pillRadius: CGFloat = 6
        static let pillTextSize: CGFloat = 11
        static let pillLetterSpacing: CGFloat = 0.5
        static let textSize: CGFloat = 11
    }
}

extension ConnectionPhase {
    /// The card writes the phase in its own palette: the title carries the
    /// hue, and the well behind the eye is a quiet fill of it. Desktop's
    /// `phaseCardColors` (`connection-phase.ts`).
    var cardTitleToken: ConnectSurfaceToken {
        switch self {
        case .exposed: .exposed
        case .connecting, .interrupted: .connecting
        case .protected: .protected
        case .blocked: .text
        }
    }

    var cardWellToken: ConnectSurfaceToken {
        switch self {
        case .exposed: .exposedWell
        case .connecting, .interrupted: .connectingWell
        case .protected: .protectedWell
        case .blocked: .button
        }
    }
}

extension ConnectionViewViewModel {
    /// The action button is filled with what the tap does. The blocked state
    /// offers a cancel that disconnects, and says so, so it takes the
    /// disconnect fill with the word.
    var actionButtonFillToken: ConnectSurfaceToken {
        switch actionButton {
        case .connect:
            .connect
        case .disconnect:
            .disconnect
        case .cancel:
            tunnelStatus.state == .waitingForConnectivity(.noConnection) ? .disconnect : .cancel
        }
    }
}

extension View {
    /// A system font at a size from the shared table, scaled with Dynamic Type
    /// like the text style it stands in for, and given the table's line height
    /// on a single line (SwiftUI has no line height of its own).
    func connectFont(
        size: CGFloat,
        weight: Font.Weight = .regular,
        lineHeight: CGFloat? = nil,
        relativeTo textStyle: Font.TextStyle = .body
    ) -> some View {
        modifier(ConnectFont(size: size, weight: weight, lineHeight: lineHeight, textStyle: textStyle))
    }
}

private struct ConnectFont: ViewModifier {
    @ScaledMetric private var size: CGFloat
    @ScaledMetric private var lineHeight: CGFloat
    private let hasLineHeight: Bool
    private let weight: Font.Weight

    init(size: CGFloat, weight: Font.Weight, lineHeight: CGFloat?, textStyle: Font.TextStyle) {
        _size = ScaledMetric(wrappedValue: size, relativeTo: textStyle)
        _lineHeight = ScaledMetric(wrappedValue: lineHeight ?? size, relativeTo: textStyle)
        hasLineHeight = lineHeight != nil
        self.weight = weight
    }

    func body(content: Content) -> some View {
        let natural = UIFont.systemFont(ofSize: size, weight: weight.uiFontWeight).lineHeight
        let leading = hasLineHeight ? max(lineHeight - natural, 0) : 0
        content
            .font(.system(size: size, weight: weight))
            .lineSpacing(leading)
            .padding(.vertical, leading / 2)
    }
}

private extension Font.Weight {
    var uiFontWeight: UIFont.Weight {
        switch self {
        case .bold: .bold
        case .semibold: .semibold
        case .medium: .medium
        default: .regular
        }
    }
}
