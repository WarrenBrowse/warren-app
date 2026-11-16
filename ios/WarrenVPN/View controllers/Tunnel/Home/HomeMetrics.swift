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
        static let logoHeight: CGFloat = 37
        static let logoInset: CGFloat = 6
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
        static let icon: CGFloat = 23
        static let badge: CGFloat = 8
        static let badgeInset: CGFloat = 7
        static let badgeRing: CGFloat = 2
        static let blur: CGFloat = 10
    }

    enum Notice {
        static let marginTop: CGFloat = 10
        static let maxWidth: CGFloat = 250
        static let radius: CGFloat = 14
        static let blur: CGFloat = 12
        static let gap: CGFloat = 10
        static let paddingVertical: CGFloat = 9
        static let paddingStart: CGFloat = 12
        static let paddingEnd: CGFloat = 10
        static let buttonInset: CGFloat = 8
        static let icon: CGFloat = 20
        static let close: CGFloat = 18
        static let titleSize: CGFloat = 13
        static let titleWeight: CGFloat = 600
        static let bodySize: CGFloat = 11
        static let bodyGap: CGFloat = 1
        static let buttonHeight: CGFloat = 30
        static let buttonPaddingHorizontal: CGFloat = 12
        static let buttonRadius: CGFloat = 9
        static let buttonTextSize: CGFloat = 12
        static let buttonTextWeight: CGFloat = 600
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
        static let marginTop: CGFloat = 2
        static let paddingVertical: CGFloat = 4
        static let lineHeight: CGFloat = 20
        static let textSize: CGFloat = 13
        static let monoSize: CGFloat = 12
    }

    enum ExitRow {
        static let marginTop: CGFloat = 2
        static let outset: CGFloat = 8
        static let paddingVertical: CGFloat = 4
        static let radius: CGFloat = 8
        static let gap: CGFloat = 10
        static let textSize: CGFloat = 12
        static let ring: CGFloat = 11
        static let ringHole: CGFloat = 3
        static let person: CGFloat = 15
        static let partGap: CGFloat = 4
        static let chevron: CGFloat = 20
        static let chevronMs: CGFloat = 300
    }

    enum Details {
        static let marginTop: CGFloat = 6
        static let radius: CGFloat = 12
        static let paddingVertical: CGFloat = 12
        static let paddingHorizontal: CGFloat = 14
        static let columnGap: CGFloat = 16
        static let rowGap: CGFloat = 7
        static let textSize: CGFloat = 12
        static let valueWeight: CGFloat = 500
        static let monoSize: CGFloat = 11
        static let dividerMargin: CGFloat = 2
        static let chipSize: CGFloat = 10
        static let chipWeight: CGFloat = 600
        static let chipPaddingVertical: CGFloat = 1
        static let chipPaddingHorizontal: CGFloat = 6
    }

    enum Controls {
        static let marginTop: CGFloat = 16
        static let gap: CGFloat = 8
        static let rowHeight: CGFloat = 46
        static let radius: CGFloat = 10
        static let thumb: CGFloat = 28
        static let thumbLine: CGFloat = 1
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

    /// The widest the header, panel and footer column grows on a large screen. Not a shared token:
    /// desktop's window is 380 wide, so only a tablet ever needs a cap.
    static let maxColumnWidth: CGFloat = 480

    /// The column on a screen this wide, the width the hero's height follows.
    static func columnWidth(forWidth width: CGFloat) -> CGFloat {
        min(width, maxColumnWidth)
    }
}

/// How the home shares a screen between the hero, the panel and the footer. The panel keeps its
/// natural height right under the hero and the footer is pinned at the bottom, so the hero is what
/// takes the free height: at least the mockup's proportion of the column, at most
/// `SceneryFraming.heroMaxHeightRatio` of it, and only what is left beyond that becomes the gap
/// above the footer. A screen too short for the panel under the minimum hero lets the hero go down
/// to the compact proportion, framed the compact way, and below that the panel scrolls. The open
/// details always take the compact hero.
struct HomeLayout: Equatable {
    let heroHeight: CGFloat
    /// The art is framed the compact way (width-fit, span top under the header).
    let compactFraming: Bool
    /// The room left between the chips and the footer.
    let gap: CGFloat
    /// The panel does not fit and scrolls above the footer.
    let scrolls: Bool

    /// `height`: the home's own height, the status bar included and the home indicator not.
    /// `panelHeight`: the panel's natural height, its overlap of the hero included.
    /// `footerHeight`: the account band's height above the home indicator.
    static func resolve(
        width: CGFloat,
        height: CGFloat,
        safeTop: CGFloat,
        panelHeight: CGFloat,
        footerHeight: CGFloat,
        detailsOpen: Bool
    ) -> HomeLayout {
        let column = HomeMetrics.columnWidth(forWidth: width)
        let minimum = SceneryFraming.heroHeight(columnWidth: column, safeTop: safeTop)
        let maximum = SceneryFraming.heroHeight(
            columnWidth: column, safeTop: safeTop, ratio: SceneryFraming.heroMaxHeightRatio)
        let compact = SceneryFraming.heroHeight(
            columnWidth: column, safeTop: safeTop, ratio: SceneryFraming.compactHeightRatio)
        let room = height - footerHeight - (panelHeight - HomeMetrics.Panel.overlap)
        let hero = detailsOpen ? compact : min(maximum, max(compact, room))
        return HomeLayout(
            heroHeight: hero,
            compactFraming: detailsOpen || hero < minimum - 0.5,
            gap: max(0, room - hero),
            scrolls: room < hero - 0.5
        )
    }
}
