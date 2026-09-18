//
//  SceneryLayout.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  Where each scenery layer lands on a screen, the one formula all three
//  clients compute. The shared fixture
//  `fixtures/client-rules/scenery_layout.json` carries it, its provenance and
//  the cases every client replays; the Android copy is `SceneryLayout.kt`.
//

import CoreGraphics
import Foundation

enum SceneryLayout {
    /// The canvas every master is painted on.
    static let canvasWidth: CGFloat = 1140
    static let canvasHeight: CGFloat = 1706

    /// Bula's painted body bottom (`bula.png`, last row with alpha >= 128). His cast shadow is a
    /// separate flat-alpha tail reaching row 1326, and aiming the slide at that tail is what used
    /// to place his feet 216 canvas rows above where the code said it wanted them.
    static let feetRow: CGFloat = 1124

    /// The first canvas row the burrow layer paints fully opaque across the whole width
    /// (`terrier.png`, first row whose every column has alpha >= 250, contiguous to the last row).
    /// Everything below it is one uniform watercolour meadow, which is why scaling only those rows
    /// continues the ground with no seam and no repetition.
    static let groundRow: CGFloat = 1301

    /// The last canvas row of the meadow before the wash runs out into bare paper: the band's row
    /// mean climbs 34 levels of 255 over rows 1681 to 1705. The band is scaled so this row lands on
    /// the screen's bottom edge and the paper rows fall below it.
    static let meadowEndRow: CGFloat = 1670

    /// Canvas columns dropped from each side of the stretched band. The watercolour fades into
    /// paper at the canvas edges, which the blurred band this replaced used to hide: measured on a
    /// 1080x2400 screen, the leftmost screen column read 1.60 times the mid-frame brightness, and
    /// dropping 20 columns brings it to 0.98. The right side fades over about 60 columns and is the
    /// hill's own sunlit edge rather than an artifact, so it is left alone.
    static let bandOverscanColumns: CGFloat = 20

    /// Air kept between Bula's feet and the card's top edge.
    static let gap: CGFloat = 16

    /// How far the whole canvas may pan up. It is the band the header already draws over, so a pan
    /// can only ever crop rows the header was covering.
    static let maxCanvasPan: CGFloat = 96

    /// The resolved placement, in points from the top of the backdrop.
    ///
    /// `canvasPan` is never positive and `foregroundShift` never negative, and exactly one of them
    /// is ever non-zero: the foreground slides DOWN over the landscape to meet a card that sits
    /// low, and when the feet would instead have to rise, the WHOLE canvas pans up by that much.
    /// Panning keeps the three layers registered, so it cannot uncover the rows the burrow exists
    /// to hide, which a bare lift did.
    struct Placement: Equatable {
        let scale: CGFloat
        let canvasHeight: CGFloat
        let canvasPan: CGFloat
        let foregroundShift: CGFloat
        let landscapeTop: CGFloat
        let foregroundTop: CGFloat
        let landscapeBottom: CGFloat
        let foregroundBottom: CGFloat
        let bandLeft: CGFloat
        let bandWidth: CGFloat
        let bandHeight: CGFloat

        /// The canvas row the two-part draw splits at, in points from a layer's own top.
        var groundOffset: CGFloat { SceneryLayout.groundRow * scale }

        /// The whole canvas at its natural scale, from a given top edge.
        func canvasRect(width: CGFloat, top: CGFloat) -> CGRect {
            CGRect(x: 0, y: top, width: width, height: canvasHeight)
        }

        /// The stretched meadow, drawn wider and taller than it needs so the painter's paper
        /// margin at the canvas edges falls off screen.
        var bandRect: CGRect {
            CGRect(
                x: bandLeft, y: foregroundTop + groundOffset,
                width: bandWidth, height: bandHeight)
        }
    }

    /// `cardTop` is the connection card's top edge in the backdrop's own coordinates, or nil
    /// before the first layout, when there is nothing to track and the canvas sits where it is
    /// painted rather than guessing.
    static func placement(in bounds: CGRect, cardTop: CGFloat?) -> Placement {
        let scale = bounds.width / canvasWidth
        let height = canvasHeight * scale
        let feetY = feetRow * scale
        let groundY = groundRow * scale
        let want = cardTop.map { $0 - gap - feetY } ?? 0
        let panNeeded = min(maxCanvasPan, max(0, -want))
        // Spelled out rather than negated in place: negating a zero yields -0.0, which reads as a
        // pan in a log and is not equal to 0.
        let canvasPan = panNeeded == 0 ? 0 : -panNeeded
        // The slide is capped so the burrow layer's opaque rows always still overlap the landscape
        // above them. Past that cap a window would open between the landscape's bottom edge and
        // the row the burrow turns opaque at, and the only ways to close it are to stretch the
        // landscape, whose lower rows are a canal and its banks rather than a uniform wash, or to
        // repeat its last row, which is pale paper at the edges and reads as a white strip.
        let foregroundShift = min(height - groundY, max(0, want - canvasPan))
        let foregroundTop = canvasPan + foregroundShift
        // The band is scaled so meadowEndRow lands on the screen's bottom edge; the paper rows
        // under it are drawn past that edge and clipped. It is never compressed, so on a screen the
        // canvas already covers it draws at its natural size.
        let bandNatural = height - groundY
        let bandNeeded = max(0, bounds.height - (foregroundTop + groundY))
        let bandHeight = max(
            bandNatural,
            bandNeeded * (canvasHeight - groundRow) / (meadowEndRow - groundRow))
        let bandScaleX = bounds.width / (canvasWidth - 2 * bandOverscanColumns)
        return Placement(
            scale: scale,
            canvasHeight: height,
            canvasPan: canvasPan,
            foregroundShift: foregroundShift,
            landscapeTop: canvasPan,
            foregroundTop: foregroundTop,
            // The landscape is never stretched: it is drawn whole, and everything below it is the
            // burrow layer's own opaque ground.
            landscapeBottom: canvasPan + height,
            foregroundBottom: max(foregroundTop + height, bounds.height),
            bandLeft: -bandOverscanColumns * bandScaleX,
            bandWidth: canvasWidth * bandScaleX,
            bandHeight: bandHeight
        )
    }
}
