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

    /// The cameras and the whole of Bula (his feet are the last row of the rabbit layer with
    /// alpha >= 128); the burrow mouth lies inside.
    static let groundSpan = Span(top: 320, bottom: 1131)

    /// Once Bula is in, the skyline down to the burrow mouth he hides in.
    static let skyBottomRow: CGFloat = 930

    static let thumbMarginRows: CGFloat = 40

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
    /// `fade`: the bottom band where the art fades into the panel.
    static func frame(width: CGFloat, height: CGFloat, clearTop: CGFloat, fade: CGFloat, span: Span) -> Frame {
        let clearBottom = height - fade
        // A hero shorter than the header leaves no room at all; the canvas then shrinks to a
        // sliver rather than flipping upside down on a negative scale.
        let room = max(clearBottom - clearTop, 1)
        let scale = min(width / canvasWidth, room / (span.bottom - span.top))
        let yMin = clearTop - span.top * scale
        let yMax = clearBottom - span.bottom * scale
        let top = min(yMax, max(yMin, 0))
        return Frame(
            scale: scale,
            left: (width - canvasWidth * scale) / 2,
            top: top,
            width: canvasWidth * scale,
            height: canvasHeight * scale,
            headroom: max(0, top)
        )
    }

    /// The tallest a hero may be at a given width: the width-fit canvas. With the canvas top at or
    /// above the hero top it then always reaches the fade, and with headroom it always reaches the
    /// hero bottom, so nothing below the canvas is ever left to fill.
    static func maxHeroHeight(forWidth width: CGFloat) -> CGFloat {
        canvasHeight * width / canvasWidth
    }

    /// The span a phase frames: the ground whenever Bula is out, the landscape's skyline down to
    /// the burrow mouth once he is in.
    static func span(skyFraming: Bool, skylineTopRow: Int) -> Span {
        skyFraming ? Span(top: CGFloat(skylineTopRow), bottom: skyBottomRow) : groundSpan
    }

    /// The round location thumbnail is a full-width square of the landscape whose top sits just
    /// above the skyline, so the landmark crowns it.
    static func thumbnailTopRow(skylineTopRow: Int) -> CGFloat {
        min(canvasHeight - canvasWidth, max(0, CGFloat(skylineTopRow) - thumbMarginRows))
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
