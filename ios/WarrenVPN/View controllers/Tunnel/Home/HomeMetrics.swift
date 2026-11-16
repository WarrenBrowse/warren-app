//
//  HomeMetrics.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  The home screen's geometry: a copy of `design-tokens.json` `home` at the
//  repository root, which desktop generates. dp and sp are points here, ms are
//  milliseconds. iOS has no generator, so `HomeMetricsTests` reads that file
//  and fails on any drift.
//

import CoreGraphics
import Foundation

enum HomeMetrics {
    enum Hero {
        static let headerInset: CGFloat = 14
        static let clearMargin: CGFloat = 6
        static let logoHeight: CGFloat = 32
        static let scrimStop: CGFloat = 0.34
        static let glowWidth: CGFloat = 300
        static let glowHeight: CGFloat = 150
        static let frameMs: CGFloat = 900
        static let crossfadeMs: CGFloat = 700
        static let bulaMs: CGFloat = 550
        static let blurRadius: CGFloat = 14
    }

    enum Toolbar {
        static let height: CGFloat = 48
        static let radius: CGFloat = 14
        static let paddingHorizontal: CGFloat = 6
        static let button: CGFloat = 38
        static let icon: CGFloat = 22
        static let badge: CGFloat = 8
        static let badgeRing: CGFloat = 2
        static let blur: CGFloat = 10
    }

    enum Panel {
        static let paddingHorizontal: CGFloat = 22
        static let overlap: CGFloat = 18
        static let gap: CGFloat = 4
    }

    enum Pill {
        static let paddingVertical: CGFloat = 7
        static let paddingStart: CGFloat = 10
        static let paddingEnd: CGFloat = 13
        static let gap: CGFloat = 7
        static let icon: CGFloat = 16
        static let textSize: CGFloat = 13
        static let textWeight: CGFloat = 700
    }

    enum Title {
        static let marginTop: CGFloat = 8
        static let textSize: CGFloat = 26
        static let textWeight: CGFloat = 600
        static let letterSpacingEm: CGFloat = -0.01
    }

    enum Subtitle {
        static let textSize: CGFloat = 13
        static let monoSize: CGFloat = 12
    }

    enum Controls {
        static let marginTop: CGFloat = 16
        static let gap: CGFloat = 8
        static let rowHeight: CGFloat = 46
        static let radius: CGFloat = 10
        static let thumb: CGFloat = 28
        static let paddingStart: CGFloat = 10
        static let paddingEnd: CGFloat = 14
        static let rowGap: CGFloat = 10
        static let labelSize: CGFloat = 15
        static let labelWeight: CGFloat = 500
        static let changeSize: CGFloat = 13
        static let buttonHeight: CGFloat = 50
        static let buttonTextSize: CGFloat = 15
        static let buttonTextWeight: CGFloat = 600
        static let outlineWidth: CGFloat = 1
    }

    enum Chips {
        static let marginTop: CGFloat = 12
        static let height: CGFloat = 23
        static let gap: CGFloat = 6
        static let textSize: CGFloat = 11
        static let paddingVertical: CGFloat = 3
        static let paddingHorizontal: CGFloat = 9
        static let borderWidth: CGFloat = 1
    }

    enum Footer {
        static let paddingVertical: CGFloat = 12
        static let paddingHorizontal: CGFloat = 22
        static let textSize: CGFloat = 12
        static let icon: CGFloat = 16
        static let gap: CGFloat = 6
    }

    /// Below this offset from the hero top nothing floats over the art: the toolbar's bottom edge
    /// plus a margin. `safeTop` is the status bar the header sits under.
    static func clearTop(safeTop: CGFloat) -> CGFloat {
        safeTop + Hero.headerInset + Toolbar.height + Hero.clearMargin
    }

    /// The widest the panel column grows on a large screen. Not a shared token: desktop's window is
    /// 360 wide, so only a tablet ever needs a cap.
    static let maxColumnWidth: CGFloat = 480

    /// The panel column on a screen this wide, the width the hero's height follows.
    static func columnWidth(forWidth width: CGFloat) -> CGFloat {
        min(width, maxColumnWidth)
    }
}
