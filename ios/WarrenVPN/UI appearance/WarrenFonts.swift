//
//  WarrenFonts.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  The app's faces: Instrument Sans for text, JetBrains Mono for keys, hosts
//  and addresses, the pair desktop sets. Both are bundled (UIAppFonts) under
//  the SIL Open Font License; the files and their licences sit in
//  `Supporting Files/Fonts`. Neither covers Cyrillic, Arabic or the CJK and
//  Southeast Asian scripts: the system font cascade draws those glyphs.
//
//  Every size scales with Dynamic Type through `UIFontMetrics`, against the
//  text style it stands in for.
//

import SwiftUI
import UIKit

enum WarrenFontFace: String, CaseIterable {
    case regular = "InstrumentSans-Regular"
    case medium = "InstrumentSans-Medium"
    case semibold = "InstrumentSans-SemiBold"
    case bold = "InstrumentSans-Bold"
    case monoRegular = "JetBrainsMonoRoman-Regular"
    case monoMedium = "JetBrainsMonoRoman-Medium"

    /// The bundled face nearest a weight. The family ships four weights, so
    /// anything lighter than regular reads as regular and anything heavier than
    /// bold as bold.
    static func sans(_ weight: UIFont.Weight) -> WarrenFontFace {
        switch weight.rawValue {
        case ..<UIFont.Weight.medium.rawValue: .regular
        case ..<UIFont.Weight.semibold.rawValue: .medium
        case ..<UIFont.Weight.bold.rawValue: .semibold
        default: .bold
        }
    }

    static func mono(_ weight: UIFont.Weight) -> WarrenFontFace {
        weight.rawValue < UIFont.Weight.medium.rawValue ? .monoRegular : .monoMedium
    }
}

extension UIFont {
    /// The size a text style has at the default content size, the base the
    /// face is scaled up or down from.
    static func defaultSize(of textStyle: UIFont.TextStyle) -> CGFloat {
        UIFontDescriptor.preferredFontDescriptor(
            withTextStyle: textStyle,
            compatibleWith: UITraitCollection(preferredContentSizeCategory: .large)
        ).pointSize
    }

    /// Instrument Sans at `size`, scaled with Dynamic Type like `textStyle`.
    static func instrumentSans(
        size: CGFloat,
        weight: UIFont.Weight = .regular,
        relativeTo textStyle: UIFont.TextStyle = .body
    ) -> UIFont {
        scaled(.sans(weight), size: size, weight: weight, textStyle: textStyle)
    }

    /// Instrument Sans in place of a text style, at that style's size.
    static func instrumentSans(_ textStyle: UIFont.TextStyle, weight: UIFont.Weight = .regular) -> UIFont {
        instrumentSans(size: defaultSize(of: textStyle), weight: weight, relativeTo: textStyle)
    }

    /// JetBrains Mono at `size`, scaled with Dynamic Type like `textStyle`.
    static func jetBrainsMono(
        size: CGFloat,
        weight: UIFont.Weight = .regular,
        relativeTo textStyle: UIFont.TextStyle = .body
    ) -> UIFont {
        scaled(.mono(weight), size: size, weight: weight, textStyle: textStyle)
    }

    private static func scaled(
        _ face: WarrenFontFace,
        size: CGFloat,
        weight: UIFont.Weight,
        textStyle: UIFont.TextStyle
    ) -> UIFont {
        // Missing only if the bundle lost the file; the system face at the same
        // size keeps the screen readable rather than crashing it.
        let base = UIFont(name: face.rawValue, size: size) ?? .systemFont(ofSize: size, weight: weight)
        return UIFontMetrics(forTextStyle: textStyle).scaledFont(for: base)
    }
}

extension Font {
    /// Instrument Sans at `size`, scaled with Dynamic Type like `textStyle`.
    static func instrumentSans(
        size: CGFloat,
        weight: UIFont.Weight = .regular,
        relativeTo textStyle: Font.TextStyle = .body
    ) -> Font {
        .custom(WarrenFontFace.sans(weight).rawValue, size: size, relativeTo: textStyle)
    }

    /// Instrument Sans in place of a text style, at that style's size.
    static func instrumentSans(_ textStyle: Font.TextStyle, weight: UIFont.Weight = .regular) -> Font {
        instrumentSans(size: UIFont.defaultSize(of: textStyle.uiTextStyle), weight: weight, relativeTo: textStyle)
    }

    /// JetBrains Mono at `size`, scaled with Dynamic Type like `textStyle`.
    static func jetBrainsMono(
        size: CGFloat,
        weight: UIFont.Weight = .regular,
        relativeTo textStyle: Font.TextStyle = .body
    ) -> Font {
        .custom(WarrenFontFace.mono(weight).rawValue, size: size, relativeTo: textStyle)
    }
    /// JetBrains Mono in place of a text style, at that style's size.
    static func jetBrainsMono(_ textStyle: Font.TextStyle, weight: UIFont.Weight = .regular) -> Font {
        jetBrainsMono(size: UIFont.defaultSize(of: textStyle.uiTextStyle), weight: weight, relativeTo: textStyle)
    }
}

extension Font.TextStyle {
    var uiTextStyle: UIFont.TextStyle {
        switch self {
        case .largeTitle: .largeTitle
        case .title: .title1
        case .title2: .title2
        case .title3: .title3
        case .headline: .headline
        case .subheadline: .subheadline
        case .callout: .callout
        case .footnote: .footnote
        case .caption: .caption1
        case .caption2: .caption2
        default: .body
        }
    }
}
