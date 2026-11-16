//
//  SceneryFraming.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  How the home hero frames the scenery, the formula every client computes.
//  The shared fixture `fixtures/client-rules/scenery_framing.json` carries it,
//  its provenance and the cases every client replays; the desktop copy is
//  `CountryBackdrop/framing.ts`.
//

import CoreGraphics
import Foundation

enum SceneryFraming {
    /// The canvas every master is painted on. The three layers are registered on it, so they are
    /// always drawn with one transform.
    static let canvasWidth: CGFloat = 1140
    static let canvasHeight: CGFloat = 1706

    /// The canvas edge facing a headroom strip or a side margin fades over this many canvas rows
    /// into the landscape's top fill, so the join does not show.
    static let featherRows: CGFloat = 48

    /// Rows of the canvas that must be fully visible between the floating header and the fade.
    struct Span: Equatable {
        let top: CGFloat
        let bottom: CGFloat
    }

    /// The cameras, the burrow and Bula down to his tail while he is out, which puts the cameras
    /// where the mockup draws them.
    static let groundSpan = Span(top: 320, bottom: 900)

    /// Once he is in, the skyline down to below the foot of its flag.
    static let skyBottomRow: CGFloat = 680

    /// The mockup's proportions (Warren VPN Accueil, 380x620): a 300 point hero on a 380 point
    /// column.
    static let heroHeightRatio: CGFloat = 300 / 380

    /// A phone hero takes free height up to this fraction of its column.
    static let heroMaxHeightRatio: CGFloat = 1.15

    /// The hero while the connection details are open (176 on the 380 window).
    static let compactHeightRatio: CGFloat = 176 / 380

    /// The hero's resize when the details open or close, on desktop's
    /// `cubic-bezier(0.3, 0.7, 0.2, 1)`.
    static let compactTransitionMs: CGFloat = 400
    static let compactTransitionCurve: (CGPoint, CGPoint) = (CGPoint(x: 0.3, y: 0.7), CGPoint(x: 0.2, y: 1))

    /// The blur of the landscape behind the side margins of a narrowed canvas, at display size.
    static let marginBlur: CGFloat = 24

    /// The hero fades into the page along the mockup's soft curve ('Doux'), as (fraction of the
    /// hero height, alpha) from its top to its bottom.
    static let maskStops: [(at: CGFloat, alpha: CGFloat)] = [
        (0, 1),
        (0.74, 1),
        (0.79, 0.9),
        (0.84, 0.7),
        (0.89, 0.45),
        (0.94, 0.2),
        (0.98, 0.06),
        (1, 0),
    ]

    /// The clear zone ends where that curve reaches half opacity; below it the art keeps fading.
    static let clearBottomFraction: CGFloat = 0.88

    /// The band under the clear zone, for a hero of this height.
    static func fade(forHeroHeight height: CGFloat) -> CGFloat {
        height * (1 - clearBottomFraction)
    }

    /// The hero keeps the mockup's proportion of the column it heads, and draws under the status
    /// bar on top of that.
    static func heroHeight(columnWidth: CGFloat, safeTop: CGFloat, ratio: CGFloat = heroHeightRatio) -> CGFloat {
        columnWidth * ratio + safeTop
    }

    /// Where the canvas lands in the hero, in points from the hero's top left corner.
    struct Frame: Equatable {
        let scale: CGFloat
        let left: CGFloat
        let top: CGFloat
        let width: CGFloat
        let height: CGFloat
        /// The strip above a canvas that had to drop to clear the header.
        let headroom: CGFloat

        var rect: CGRect { CGRect(x: left, y: top, width: width, height: height) }

        /// Where a canvas row lands in the hero.
        func y(ofRow row: CGFloat) -> CGFloat { top + row * scale }
    }

    /// `clearTop`: below this offset nothing floats over the art (the header bottom plus a margin).
    /// `fade`: the band under the clear zone, `fade(forHeroHeight:)`.
    /// `compact`: the hero is short (the details are open), so the art keeps the scale the same
    /// hero has at its full height (`fullHeight`, the hero with the details closed) and pins the span
    /// top under the header, its lower part going into the fade: opening the details slides the
    /// art up, it never zooms into it.
    static func frame(
        width: CGFloat,
        height: CGFloat,
        clearTop: CGFloat,
        fade: CGFloat,
        span: Span,
        compact: Bool = false,
        fullHeight: CGFloat? = nil
    ) -> Frame {
        let clearBottom = height - fade
        let fitBottom = compact ? (fullHeight ?? height) * clearBottomFraction : clearBottom
        // A hero shorter than the header leaves no room at all; the canvas then shrinks to a
        // sliver rather than flipping upside down on a negative scale.
        let room = max(fitBottom - clearTop, 1)
        let scale = min(width / canvasWidth, room / (span.bottom - span.top))
        let yMin = clearTop - span.top * scale
        let yMax = clearBottom - span.bottom * scale
        let top = compact ? yMin : min(yMax, max(yMin, 0))
        return Frame(
            scale: scale,
            left: (width - canvasWidth * scale) / 2,
            top: top,
            width: canvasWidth * scale,
            height: canvasHeight * scale,
            headroom: max(0, top)
        )
    }

    /// Where the blurred landscape behind a narrowed canvas lands: at cover scale over the whole
    /// hero, its span's middle row at the height the canvas puts it, so the horizon runs on across
    /// the feathered join. Nil when the canvas spans the hero's width and leaves no margin.
    static func marginBackdrop(width: CGFloat, height: CGFloat, frame: Frame, span: Span) -> CGRect? {
        guard frame.left > 0.5 else { return nil }
        let scale = max(width / canvasWidth, height / canvasHeight)
        let size = CGSize(width: canvasWidth * scale, height: canvasHeight * scale)
        let middle = (span.top + span.bottom) / 2
        let top = min(0, max(height - size.height, frame.y(ofRow: middle) - middle * scale))
        return CGRect(origin: CGPoint(x: (width - size.width) / 2, y: top), size: size)
    }

    /// The span a phase frames: the ground whenever Bula is out, the landscape's skyline down to
    /// the burrow once he is in.
    static func span(skyFraming: Bool, skylineTopRow: Int) -> Span {
        skyFraming ? Span(top: CGFloat(skylineTopRow), bottom: skyBottomRow) : groundSpan
    }

    /// The connecting blur, as a radius at display size (desktop's `blur(14px)`, Android's
    /// `LANDSCAPE_BLUR_RADIUS`), and the zoom and dim that go with it.
    static let connectingBlur: CGFloat = HomeMetrics.Hero.blurRadius
    static let connectingZoom: CGFloat = 1.08
    static let connectingDim: CGFloat = 0.08

    /// `connectingBlur` carried into canvas pixels, because iOS blurs the source image rather than
    /// the rendered view and the two only agree at one drawn width.
    static func blurRadius(forDrawnWidth width: CGFloat) -> CGFloat {
        guard width > 0 else { return connectingBlur }
        return connectingBlur * canvasWidth / width
    }
}
