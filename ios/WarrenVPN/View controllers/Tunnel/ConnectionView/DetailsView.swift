//
//  DetailsView.swift
//  MullvadVPN
//
//  Created by Andrew Bulhak on 2025-01-03.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import SwiftUI

/// One line of the connection details card (desktop `ConnectionDetails`).
enum HomeDetailsLine: Equatable {
    enum Field: Equatable {
        /// The tunnel protocol, at the card's value weight.
        case `protocol`
        /// Where the traffic enters the network, an address in the mono face.
        case entry
        /// Where it leaves, an address or a place in the mono face.
        case exit

        var label: String {
            switch self {
            case .protocol: NSLocalizedString("Protocol", comment: "")
            case .entry: NSLocalizedString("In", comment: "")
            case .exit: NSLocalizedString("Out", comment: "")
            }
        }

        var isMono: Bool { self != .protocol }

        var accessibilityIdentifier: String {
            switch self {
            case .protocol: "connectionDetailsProtocol"
            case .entry: AccessibilityIdentifier.connectionPanelInAddressRow.asString
            case .exit: AccessibilityIdentifier.connectionPanelOutAddressRow.asString
            }
        }
    }

    /// A field and its value, one value line per entry.
    case row(Field, values: [String])
    /// The hairline between the addresses and the obfuscation.
    case divider
    /// The obfuscation the tunnel uses, with its "active" chip while it runs.
    case obfuscation(active: Bool)
}

enum HomeDetails {
    /// The card's lines: the protocol (and the two hops when there are two), where the traffic
    /// goes in and comes out, then the obfuscation under a hairline. `out` is the exit endpoint
    /// when the client knows it, its location otherwise.
    static func lines(isMultihop: Bool, inAddress: String?, out: String?) -> [HomeDetailsLine] {
        var lines: [HomeDetailsLine] = [
            .row(
                .protocol,
                values: ["QUIC"] + (isMultihop ? [NSLocalizedString("Multihop (2 hops)", comment: "")] : [])
            )
        ]
        if let inAddress {
            lines.append(.row(.entry, values: [inAddress]))
        }
        if let out {
            lines.append(.row(.exit, values: [out]))
        }
        // HTTP/3 mimicry is the Warren baseline, always on: there is no state in which it is off.
        lines += [.divider, .obfuscation(active: true)]
        return lines
    }
}

/// The connection details the exit row opens in place, under itself: the mockup's card, a grid
/// of label and value with the addresses in the mono face.
struct ConnectionDetailsView: View {
    @ObservedObject var viewModel: ConnectionViewViewModel
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let palette = ConnectSurfacePalette(colorScheme: colorScheme)
        let details = HomeMetrics.Details.self
        Grid(alignment: .leading, horizontalSpacing: details.columnGap, verticalSpacing: details.rowGap) {
            ForEach(Array(viewModel.homeDetailsLines.enumerated()), id: \.offset) { _, line in
                lineView(line, palette: palette)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.vertical, details.paddingVertical)
        .padding(.horizontal, details.paddingHorizontal)
        .background(
            RoundedRectangle(cornerRadius: details.radius, style: .continuous)
                .fill(palette.color(.homeDetails))
        )
        .overlay(
            RoundedRectangle(cornerRadius: details.radius, style: .continuous)
                .strokeBorder(palette.color(.homeDetailsLine), lineWidth: 1)
        )
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("connectionDetails")
    }

    @ViewBuilder
    private func lineView(_ line: HomeDetailsLine, palette: ConnectSurfacePalette) -> some View {
        let details = HomeMetrics.Details.self
        switch line {
        case let .row(field, values):
            GridRow(alignment: .firstTextBaseline) {
                Text(verbatim: field.label)
                    .font(.instrumentSans(size: details.textSize, relativeTo: .caption))
                    .foregroundStyle(palette.color(.homeTextSecondary))
                    .lineLimit(1)
                    .fixedSize()
                    .accessibilityHidden(true)
                VStack(alignment: .leading, spacing: 2) {
                    ForEach(Array(values.enumerated()), id: \.offset) { index, value in
                        Text(verbatim: value)
                            .font(
                                field.isMono
                                    ? .jetBrainsMono(size: details.monoSize, relativeTo: .caption)
                                    : .instrumentSans(size: details.textSize, weight: .medium, relativeTo: .caption)
                            )
                            .foregroundStyle(palette.color(.homeText))
                            // An address reads left to right in every language.
                            .environment(\.layoutDirection, field.isMono ? .leftToRight : layoutDirection)
                            // The first line speaks for the row, "In 1.2.3.4:443 UDP".
                            .accessibilityLabel(Text(verbatim: "\(field.label) \(values.joined(separator: ", "))"))
                            .accessibilityIdentifier(index == 0 ? field.accessibilityIdentifier : "")
                            .accessibilityHidden(index > 0)
                    }
                }
            }
        case .divider:
            Rectangle()
                .fill(palette.color(.homeDetailsLine))
                .frame(height: 1)
                .padding(.vertical, details.dividerMargin)
                .gridCellColumns(2)
                // The hairline spans the card without widening the label column.
                .gridCellUnsizedAxes(.horizontal)
                .accessibilityHidden(true)
        case let .obfuscation(active):
            GridRow(alignment: .firstTextBaseline) {
                Text(LocalizedStringKey("Obfuscation"))
                    .font(.instrumentSans(size: details.textSize, relativeTo: .caption))
                    .foregroundStyle(palette.color(.homeTextSecondary))
                    .lineLimit(1)
                    .fixedSize()
                HStack(spacing: 6) {
                    Text(LocalizedStringKey("HTTP/3 mimicry"))
                        .font(.instrumentSans(size: details.textSize, relativeTo: .caption))
                        .foregroundStyle(palette.color(.homeText))
                    if active {
                        Text(LocalizedStringKey("active"))
                            .font(.instrumentSans(size: details.chipSize, weight: .semibold, relativeTo: .caption2))
                            .foregroundStyle(palette.color(.homeActive))
                            .padding(.vertical, details.chipPaddingVertical)
                            .padding(.horizontal, details.chipPaddingHorizontal)
                            .background(Capsule().fill(palette.color(.homeActiveWell)))
                    }
                }
            }
            .accessibilityElement(children: .combine)
        }
    }

    @Environment(\.layoutDirection) private var layoutDirection
}
