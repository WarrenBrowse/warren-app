import SwiftUI

// The app's named text styles, set in Instrument Sans at each text style's size
// and scaled with Dynamic Type (`WarrenFonts.swift`).

extension Font {
    static var warrenBig: Font { .instrumentSans(.largeTitle, weight: .bold) }
    static var warrenLarge: Font { .instrumentSans(.title, weight: .bold) }
    static var warrenMedium: Font { .instrumentSans(.title3, weight: .semibold) }
    static var warrenSmall: Font { .instrumentSans(.body) }
    static var warrenSmallSemiBold: Font { .instrumentSans(.body, weight: .semibold) }
    static var warrenTiny: Font { .instrumentSans(.subheadline) }
    static var warrenTinySemiBold: Font { .instrumentSans(.subheadline, weight: .semibold) }
    static var warrenMini: Font { .instrumentSans(.footnote) }
    static var warrenMiniSemiBold: Font { .instrumentSans(.footnote, weight: .semibold) }
    static var warrenMicro: Font { .instrumentSans(.caption2) }
    static var warrenMicroSemiBold: Font { .instrumentSans(.caption2, weight: .semibold) }
}

extension UIFont {
    static var warrenBig: UIFont { .instrumentSans(.largeTitle, weight: .bold) }
    static var warrenLarge: UIFont { .instrumentSans(.title1, weight: .bold) }
    static var warrenMedium: UIFont { .instrumentSans(.title3, weight: .semibold) }
    static var warrenSmall: UIFont { .instrumentSans(.body) }
    static var warrenSmallSemiBold: UIFont { .instrumentSans(.body, weight: .semibold) }
    static var warrenTiny: UIFont { .instrumentSans(.subheadline) }
    static var warrenTinySemiBold: UIFont { .instrumentSans(.subheadline, weight: .semibold) }
    static var warrenMini: UIFont { .instrumentSans(.footnote) }
    static var warrenMiniSemiBold: UIFont { .instrumentSans(.footnote, weight: .semibold) }
    static var warrenMicro: UIFont { .instrumentSans(.caption2) }
    static var warrenMicroSemiBold: UIFont { .instrumentSans(.caption2, weight: .semibold) }
}
