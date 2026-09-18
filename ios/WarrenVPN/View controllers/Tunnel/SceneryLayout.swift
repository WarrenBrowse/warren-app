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

    /// The top of the country flag: the highest thing on the canvas that has to stay on screen.
    /// Measured on the flag's own fabric (Finland rows 463 to 531), with headroom, and the other
    /// flagged countries are drawn to the same template.
    static let flagTopRow: CGFloat = 430

    /// Air kept between Bula's feet and the card's top edge.
    static let gap: CGFloat = 16

    /// How far the whole canvas may pan up. It is the band the header already draws over, so a pan
    /// can only ever crop rows the header was covering.
    static let maxCanvasPan: CGFloat = 96

    /// The connecting blur, as a radius at DISPLAY size: desktop's `blur(14px)` on its 400 px
    /// window, Android's `LANDSCAPE_BLUR_RADIUS = 14.dp`.
    static let connectingBlur: CGFloat = 14

    /// The connecting zoom and dim, the same on all three clients.
    static let connectingZoom: CGFloat = 1.08
    static let connectingDim: CGFloat = 0.08

    /// [connectingBlur] carried into canvas pixels, because iOS blurs the source image rather than
    /// the rendered view and the two only agree at one screen width. A fixed canvas-space radius
    /// stood here and was right at no width in particular: 0.75 of the other two clients on a
    /// 393 pt phone, 0.60 on a 320 pt one, and 1.9 times too strong on a 1024 pt iPad.
    static func blurRadius(forWidth width: CGFloat) -> CGFloat {
        guard width > 0 else { return connectingBlur }
        return connectingBlur * canvasWidth / width
    }

    /// The resolved placement, in points from the top of the backdrop.
    ///
    /// `canvasPan` is never positive and `foregroundShift` never negative, and exactly one of them
    /// is ever non-zero: the foreground slides DOWN over the landscape to meet a card that sits
    /// low, and when the feet would instead have to rise, the WHOLE canvas pans up by that much.
    /// Panning keeps the three layers registered, so it cannot uncover the rows the burrow exists
    /// to hide, which a bare lift did.
    struct Placement: Equatable {
        let scale: CGFloat
        /// How wide the canvas is actually drawn, and where its left edge sits. On every portrait
        /// phone this is the full screen width at x 0; on a short screen the canvas is narrower and
        /// centred, and the margins either side take its own edge colour.
        let canvasWidth: CGFloat
        let canvasLeft: CGFloat
        let canvasHeight: CGFloat
        /// Whether Bula and the burrow are drawn at all. False only where they cannot clear the
        /// connection card, which today is a landscape phone.
        let showsForeground: Bool
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
        func canvasRect(top: CGFloat) -> CGRect {
            CGRect(x: canvasLeft, y: top, width: canvasWidth, height: canvasHeight)
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
        // Fitting the width is a MAXIMUM, not the rule. On any landscape geometry it puts the span
        // from the flag down to Bula's feet taller than the room above the card (an iPad 11 in
        // landscape wants 727 pt of the 485 it has), and the pan cap then leaves his feet below the
        // bottom edge with the flag cropped off the top anyway. So the canvas shrinks until that
        // span fits, and is centred. On every portrait phone the width term wins and nothing here
        // changes.
        let widthFit = bounds.width / canvasWidth
        let heightFit = bounds.height / canvasHeight
        let room = cardTop.map { $0 - gap } ?? bounds.height
        let spanFit = max(0, room) / (feetRow - flagTopRow)
        // Never below height-fit: a landscape phone leaves 84 pt above the card, and fitting the
        // span into that would draw the canvas as a 293 px strip on a 2400 px screen. The canvas
        // always covers the screen; where it then cannot clear the card, the foreground is not
        // drawn at all rather than half buried (see `showsForeground`).
        let scale = min(widthFit, max(spanFit, heightFit))
        let drawnWidth = canvasWidth * scale
        let canvasLeft = (bounds.width - drawnWidth) / 2
        let height = canvasHeight * scale
        let feetY = feetRow * scale
        let groundY = groundRow * scale
        let want = cardTop.map { $0 - gap - feetY } ?? 0
        // The cap is the sky above the flag, or the header band, whichever is more generous.
        // Panning by flagTopRow crops only rows the flag sits below, which is what that constant
        // means, so it can never hide something that has to stay in frame; on a portrait phone the
        // header band is the wider of the two and this reads as it always did.
        let panLimit = max(maxCanvasPan, flagTopRow * scale)
        let panNeeded = min(panLimit, max(0, -want))
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
        // Bula and the burrow ride the same layer, so either both clear the card or neither is
        // drawn. A landscape phone is the case that cannot: the card leaves 84 pt, and the choice
        // there is between a rabbit sunk to the ears behind it and the country art alone. The art
        // alone is a composition; the sunk rabbit is an accident.
        let showsForeground = (foregroundTop + feetY) <= (cardTop ?? bounds.height) + 0.5
        // The band is scaled so meadowEndRow lands on the screen's bottom edge; the paper rows
        // under it are drawn past that edge and clipped. It is never compressed, so on a screen the
        // canvas already covers it draws at its natural size.
        let bandNatural = height - groundY
        let bandNeeded = max(0, bounds.height - (foregroundTop + groundY))
        let bandHeight = max(
            bandNatural,
            bandNeeded * (canvasHeight - groundRow) / (meadowEndRow - groundRow))
        // The band spans the drawn canvas, inset past the paper margin, so on a short screen it
        // stops with the canvas rather than running under the side margins.
        let bandScaleX = drawnWidth / (canvasWidth - 2 * bandOverscanColumns)
        return Placement(
            scale: scale,
            canvasWidth: drawnWidth,
            canvasLeft: canvasLeft,
            canvasHeight: height,
            showsForeground: showsForeground,
            canvasPan: canvasPan,
            foregroundShift: foregroundShift,
            landscapeTop: canvasPan,
            foregroundTop: foregroundTop,
            // The landscape is never stretched: it is drawn whole, and everything below it is the
            // burrow layer's own opaque ground.
            landscapeBottom: canvasPan + height,
            foregroundBottom: max(foregroundTop + height, bounds.height),
            bandLeft: canvasLeft - bandOverscanColumns * bandScaleX,
            bandWidth: canvasWidth * bandScaleX,
            bandHeight: bandHeight
        )
    }
}
