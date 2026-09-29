//
//  CardButton.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import SwiftUI

/// The buttons of the connection card, desktop's `CardButton`.
///
/// `neutral` is the location and shuffle buttons, a raised fill of the card
/// itself. The action tones are white on a fill that says what the tap does,
/// never the state the tunnel is in.
struct CardButtonStyle: ButtonStyle {
    enum Tone {
        case neutral
        case action(ConnectSurfaceToken)
    }

    let tone: Tone

    @Environment(\.colorScheme) private var colorScheme
    @Environment(\.isEnabled) private var isEnabled

    func makeBody(configuration: Configuration) -> some View {
        let palette = ConnectSurfacePalette(colorScheme: colorScheme)
        let metrics = ConnectSurfaceMetrics.Button.self
        let shape = RoundedRectangle(cornerRadius: metrics.radius)

        configuration.label
            .connectFont(
                size: metrics.textSize,
                weight: .semibold,
                lineHeight: metrics.textLineHeight,
                relativeTo: .subheadline
            )
            .lineLimit(1)
            .foregroundStyle(palette.color(textToken))
            .opacity(isEnabled ? 1 : 0.5)
            .padding(.horizontal, 8)
            .frame(maxWidth: .infinity, minHeight: metrics.height)
            .background(shape.fill(palette.color(fillToken(isPressed: configuration.isPressed))))
            .overlay {
                if let borderToken {
                    shape.strokeBorder(palette.color(borderToken), lineWidth: metrics.borderWidth)
                }
            }
            .contentShape(shape)
    }

    private var textToken: ConnectSurfaceToken {
        switch tone {
        case .neutral: .text
        case .action: .actionText
        }
    }

    private var borderToken: ConnectSurfaceToken? {
        switch tone {
        case .neutral: .buttonLine
        case .action: nil
        }
    }

    private func fillToken(isPressed: Bool) -> ConnectSurfaceToken {
        switch tone {
        case .neutral:
            return isPressed ? .buttonPressed : .button
        case let .action(fill):
            guard isPressed else { return fill }
            return switch fill {
            case .connect: .connectPressed
            case .disconnect: .disconnectPressed
            case .cancel: .cancelPressed
            default: fill
            }
        }
    }
}
