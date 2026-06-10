import SwiftUI

extension Font {
    static var warrenBig: Font { .largeTitle.bold() }
    static var warrenLarge: Font { .title.bold() }
    static var warrenMedium: Font { .title3.weight(.semibold) }
    static var warrenSmall: Font { .body }
    static var warrenSmallSemiBold: Font { warrenSmall.weight(.semibold) }
    static var warrenTiny: Font { .subheadline }
    static var warrenTinySemiBold: Font { .warrenTiny.weight(.semibold) }
    static var warrenMini: Font { .footnote }
    static var warrenMiniSemiBold: Font { warrenMini.weight(.semibold) }
    static var warrenMicro: Font { .caption2 }
    static var warrenMicroSemiBold: Font { warrenMicro.weight(.semibold) }
}

extension UIFont {
    static var warrenBig: UIFont { .preferredFont(forTextStyle: .largeTitle, weight: .bold) }
    static var warrenLarge: UIFont { .preferredFont(forTextStyle: .title1, weight: .bold) }
    static var warrenMedium: UIFont { .preferredFont(forTextStyle: .title3, weight: .semibold) }
    static var warrenSmall: UIFont { .preferredFont(forTextStyle: .body) }
    static var warrenSmallSemiBold: UIFont { .preferredFont(forTextStyle: .body, weight: .semibold) }
    static var warrenTiny: UIFont { .preferredFont(forTextStyle: .subheadline) }
    static var warrenTinySemiBold: UIFont { .preferredFont(forTextStyle: .subheadline, weight: .semibold) }
    static var warrenMini: UIFont { .preferredFont(forTextStyle: .footnote) }
    static var warrenMiniSemiBold: UIFont { .preferredFont(forTextStyle: .footnote, weight: .semibold) }
    static var warrenMicro: UIFont { .preferredFont(forTextStyle: .caption2) }
    static var warrenMicroSemiBold: UIFont { .preferredFont(forTextStyle: .caption2, weight: .semibold) }
}
