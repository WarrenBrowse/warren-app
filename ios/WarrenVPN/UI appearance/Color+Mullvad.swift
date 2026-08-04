import SwiftUI

extension Color {
    private static let warrenPrimaryColor = MullvadBlue.base
    private static let warrenSecondaryColor = MullvadDarkBlue.base
    private static let warrenWarningColor = UIColor.warningColor.color
    static let warrenDangerColor = UIColor.dangerColor.color
    static let warrenSuccessColor = UIColor.successColor.color

    static let warrenBackground: Color = .warrenSecondaryColor
    static let warrenContainerBackground: Color = MullvadDarkBlue._10
    static let warrenDarkBackground: Color = MullvadDarkBlue._50
    static let warrenTextPrimary: Color = UIColor.primaryTextColor.color
    static let warrenTextSecondary: Color = MullvadWhite._60
    static let warrenTextPrimaryDisabled: Color = .warrenTextPrimary.opacity(
        0.2
    )
    static let secondaryTextColor: Color = UIColor.secondaryTextColor.color

    // Warren dark palette (Bula): the historical Mullvad "blue" names are kept
    // so call sites stay upstream-compatible, but the values are the neutral
    // grey ladders from the desktop source of truth (color-tokens.ts).
    private enum MullvadBlue {
        static let base: Color = .init(red: 74 / 255, green: 72 / 255, blue: 70 / 255)
        static let _10: Color = .init(red: 42 / 255, green: 41 / 255, blue: 40 / 255)
        static let _20: Color = .init(red: 48 / 255, green: 47 / 255, blue: 46 / 255)
        static let _40: Color = .init(red: 56 / 255, green: 55 / 255, blue: 53 / 255)
        static let _50: Color = .init(red: 64 / 255, green: 62 / 255, blue: 60 / 255)
        static let _60: Color = .init(red: 72 / 255, green: 70 / 255, blue: 68 / 255)
        static let _80: Color = .init(red: 80 / 255, green: 78 / 255, blue: 75 / 255)
    }

    private enum MullvadDarkBlue {
        static let base: Color = .init(red: 31 / 255, green: 31 / 255, blue: 32 / 255)
        static let _50: Color = .init(red: 26 / 255, green: 26 / 255, blue: 27 / 255)
        static let _10: Color = .init(red: 18 / 255, green: 18 / 255, blue: 19 / 255)
        static let _10_alpha80: Color = MullvadDarkBlue._10.opacity(
            0.8
        )
    }

    private enum MullvadRed {
        static let base: Color = .init(red: 202 / 255, green: 76 / 255, blue: 56 / 255)
    }

    private enum MullvadGreen {
        static let base: Color = .init(red: 110 / 255, green: 162 / 255, blue: 78 / 255)
    }

    private enum MullvadYellow {
        static let base: Color = .init(red: 202 / 255, green: 150 / 255, blue: 60 / 255)
    }

    private enum MullvadWhiteOnDarkBlue {
        static let _5: Color = .init(red: 50 / 255, green: 50 / 255, blue: 51 / 255)
    }

    private enum MullvadWhite {
        // True near-white (neutral, NOT cream).
        static let _100: Color = .init(red: 247 / 255, green: 247 / 255, blue: 248 / 255)
        static let _80: Color = _100.opacity(0.8)
        static let _60: Color = _100.opacity(0.6)
        static let _40: Color = _100.opacity(0.4)
        static let _20: Color = _100.opacity(0.2)
    }

    enum MullvadActionBox {
        static let border: Color = .warrenPrimaryColor
    }

    enum MullvadText {
        static let inputPlaceholder: Color = MullvadWhite._60
        static let disabled: Color = MullvadWhite._20
        static let onBackground: Color = MullvadWhite._60
        static let onBackgroundEmphasis100: Color = MullvadWhite._100
    }

    enum MullvadButton {
        static let primary: Color = .warrenPrimaryColor
        static let primaryPressed = Color(red: 56 / 255, green: 55 / 255, blue: 53 / 255)
        static let primaryDisabled = primaryPressed
        static let danger: Color = .warrenDangerColor
        static let dangerPressed = Color(red: 176 / 255, green: 68 / 255, blue: 52 / 255)
        static let dangerDisabled = Color(red: 100 / 255, green: 50 / 255, blue: 44 / 255)
        static let positive: Color = .warrenSuccessColor
        static let positivePressed = Color(red: 94 / 255, green: 142 / 255, blue: 66 / 255)
        static let positiveDisabled = Color(red: 58 / 255, green: 86 / 255, blue: 48 / 255)
    }

    enum MullvadList {
        static let separator: Color = .warrenSecondaryColor
        static let background: Color = .warrenPrimaryColor
        enum Item {
            static let parent: Color = .warrenPrimaryColor
            static let child1 = Color.MullvadBlue._60
            static let child2 = Color.MullvadBlue._40
            static let child3 = Color.MullvadBlue._20
            static let child4 = Color.MullvadBlue._10
        }
    }

    enum MullvadTextField {
        static let background: Color = .MullvadBlue._40
        static let backgroundDisabled: Color = .MullvadWhiteOnDarkBlue._5
        static let backgroundSuggestion: Color = .MullvadBlue._80
        static let inputPlaceholder: Color = MullvadText.inputPlaceholder
        static let textDisabled: Color = MullvadText.disabled
        static let textInput: Color = MullvadText.onBackgroundEmphasis100
        static let label: Color = MullvadText.onBackgroundEmphasis100
        static let border: Color = .MullvadOpacities.chalk40
        static let borderFocused: Color = .MullvadNewGraphicalProfile.chalk
        static let borderError: Color = .MullvadNewGraphicalProfile.red
    }

    enum MullvadDashboard {
        static let background: Color = MullvadDarkBlue._10_alpha80
    }

    private enum MullvadOpacities {
        static let chalk40: Color = .MullvadNewGraphicalProfile.chalk.opacity(
            0.4
        )
    }

    private enum MullvadNewGraphicalProfile {
        static let red: Color = .init(red: 214 / 255, green: 96 / 255, blue: 70 / 255)
        // Warm "paper" chalk: a deliberate small warm accent (mnemonic grid,
        // focused borders), the only non-neutral light surface.
        static let chalk: Color = .init(red: 244 / 255, green: 240 / 255, blue: 232 / 255)
        static let dark: Color = .init(red: 66 / 255, green: 65 / 255, blue: 64 / 255)
    }
}
