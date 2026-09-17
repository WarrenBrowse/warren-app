//
//  View+Size.swift
//  MullvadVPN
//
//  Created by Jon Petersson on 2024-11-14.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import SwiftUI

extension View {
    /// Measures view size.
    func sizeOfView(_ onSizeChange: @escaping ((CGSize) -> Void)) -> some View {
        return
            self
            .background {
                GeometryReader { proxy in
                    Color.clear
                        .preference(key: ViewSizeKey.self, value: proxy.size)
                        .onPreferenceChange(ViewSizeKey.self) { size in
                            onSizeChange(size)
                        }
                }
            }
    }

    /// Reports the view's top edge in the window's coordinate space, on every
    /// frame of a height animation. The scenery backdrop follows the
    /// connection card with it.
    func topOfView(_ onTopChange: @escaping ((CGFloat) -> Void)) -> some View {
        return
            self
            .background {
                GeometryReader { proxy in
                    Color.clear
                        .preference(key: ViewTopKey.self, value: proxy.frame(in: .global).minY)
                        .onPreferenceChange(ViewTopKey.self) { top in
                            onTopChange(top)
                        }
                }
            }
    }
}

private struct ViewSizeKey: PreferenceKey, Sendable {
    nonisolated(unsafe) static var defaultValue: CGSize = .zero

    static func reduce(value: inout CGSize, nextValue: () -> CGSize) {
        value = nextValue()
    }
}

private struct ViewTopKey: PreferenceKey, Sendable {
    nonisolated(unsafe) static var defaultValue: CGFloat = 0

    static func reduce(value: inout CGFloat, nextValue: () -> CGFloat) {
        value = nextValue()
    }
}
