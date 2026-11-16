//
//  HomeFooterView.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  The bottom of the home: the feature chips, the beta line and the account
//  footer band (desktop `FeatureIndicators`, `BetaBadge` 'home' and
//  `AppMainFooter`).
//

import SwiftUI

/// The active features as outlined chips that wrap. One row is reserved even
/// with none, so the hero does not reframe every time the tunnel comes up.
struct HomeChipsView: View {
    @ObservedObject var viewModel: FeatureIndicatorsViewModel
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let palette = ConnectSurfacePalette(colorScheme: colorScheme)
        let chips = HomeMetrics.Chips.self
        HomeFlowLayout(spacing: chips.gap) {
            ForEach(viewModel.chips) { chip in
                Button {
                    viewModel.onPressed(item: chip)
                } label: {
                    Text(verbatim: chip.name)
                        .font(.instrumentSans(size: chips.textSize, relativeTo: .caption2))
                        .foregroundStyle(palette.color(.homeChipText))
                        .lineLimit(1)
                        .padding(.vertical, chips.paddingVertical)
                        .padding(.horizontal, chips.paddingHorizontal)
                        .overlay(Capsule().strokeBorder(palette.color(.homeChipLine), lineWidth: chips.borderWidth))
                        .contentShape(Capsule())
                }
                .buttonStyle(.plain)
            }
        }
        .frame(maxWidth: .infinity, minHeight: chips.height, alignment: .topLeading)
        .padding(.horizontal, HomeMetrics.Panel.paddingHorizontal)
    }
}

/// Lays its children out in rows, wrapping to a new row when the next one
/// would overflow.
struct HomeFlowLayout: Layout {
    let spacing: CGFloat

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let rows = rows(width: proposal.width ?? .infinity, subviews: subviews)
        let height = rows.map(\.height).reduce(0, +) + spacing * CGFloat(max(rows.count - 1, 0))
        let width = rows.map(\.width).max() ?? 0
        return CGSize(width: proposal.width ?? width, height: height)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        var y = bounds.minY
        for row in rows(width: bounds.width, subviews: subviews) {
            var x = bounds.minX
            for index in row.indices {
                let size = subviews[index].sizeThatFits(.unspecified)
                subviews[index].place(at: CGPoint(x: x, y: y), proposal: ProposedViewSize(size))
                x += size.width + spacing
            }
            y += row.height + spacing
        }
    }

    private struct Row {
        var indices: [Int] = []
        var width: CGFloat = 0
        var height: CGFloat = 0
    }

    private func rows(width: CGFloat, subviews: Subviews) -> [Row] {
        var rows: [Row] = []
        var current = Row()
        for index in subviews.indices {
            let size = subviews[index].sizeThatFits(.unspecified)
            let needed = current.indices.isEmpty ? size.width : current.width + spacing + size.width
            if needed > width, !current.indices.isEmpty {
                rows.append(current)
                current = Row()
            }
            current.width = current.indices.isEmpty ? size.width : current.width + spacing + size.width
            current.height = max(current.height, size.height)
            current.indices.append(index)
        }
        if !current.indices.isEmpty {
            rows.append(current)
        }
        return rows
    }
}

/// On a non-prod build, a quiet line above the footer: the chip carries the
/// build's identity, the line its one practical consequence, and a tap
/// explains the network.
struct HomeBetaLine: View {
    let badge: String
    let line: String
    let action: () -> Void

    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let palette = ConnectSurfacePalette(colorScheme: colorScheme)
        Button(action: action) {
            HStack(spacing: 8) {
                Text(verbatim: badge)
                    .font(.instrumentSans(size: 10, weight: .bold, relativeTo: .caption2))
                    .kerning(0.5)
                    .foregroundStyle(palette.color(.homePillPendingText))
                    .padding(.horizontal, 6)
                    .frame(minHeight: 18)
                    .background(RoundedRectangle(cornerRadius: 5).fill(palette.color(.homeBadge)))
                    .fixedSize()
                Text(verbatim: line)
                    .font(.instrumentSans(size: 12, relativeTo: .caption))
                    .foregroundStyle(palette.color(.homeTextSecondary))
                    .lineLimit(1)
                    .truncationMode(.tail)
                Spacer(minLength: 0)
            }
            .padding(.horizontal, HomeMetrics.Panel.paddingHorizontal)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityLabel(
            String(
                format: NSLocalizedString("%@ build, about this network", tableName: "Settings", comment: ""),
                badge
            )
        )
        .accessibilityHint(line)
    }
}

/// The account band at the foot of the home: the shortened public key, which a
/// tap copies, and the time left.
struct HomeFooterView: View {
    @ObservedObject var model: HomeFooterModel
    @State private var justCopied = false
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let palette = ConnectSurfacePalette(colorScheme: colorScheme)
        let footer = HomeMetrics.Footer.self
        HStack(spacing: footer.gap) {
            if let pubkey = model.pubkey {
                Button {
                    UIPasteboard.general.string = pubkey
                    justCopied = true
                    Task {
                        try? await Task.sleep(nanoseconds: 2_000_000_000)
                        justCopied = false
                    }
                } label: {
                    HStack(spacing: footer.gap) {
                        Text(verbatim: Self.shorten(pubkey))
                            .font(.jetBrainsMono(size: footer.textSize, relativeTo: .caption))
                            // An address reads left to right in every language.
                            .environment(\.layoutDirection, .leftToRight)
                        Image(systemName: justCopied ? "checkmark" : "doc.on.doc")
                            .font(.system(size: footer.icon - 3))
                            .frame(width: footer.icon, height: footer.icon)
                            .foregroundStyle(
                                justCopied ? palette.color(.homePillProtected) : palette.color(.homeTextSecondary))
                    }
                }
                .buttonStyle(.plain)
                .accessibilityLabel(LocalizedStringKey("Copy public key"))
            }
            Spacer(minLength: footer.gap)
            if let timeLeft = HomeTimeLeft.text(now: Date(), expiry: model.expiry) {
                Text(verbatim: timeLeft)
                    .font(.instrumentSans(size: footer.textSize, relativeTo: .caption))
                    .lineLimit(1)
            }
        }
        .foregroundStyle(palette.color(.homeTextSecondary))
        .padding(.vertical, footer.paddingVertical)
        .padding(.horizontal, footer.paddingHorizontal)
        .frame(maxWidth: HomeMetrics.maxColumnWidth)
        .frame(maxWidth: .infinity)
        .background(palette.color(.homeFooter).ignoresSafeArea(edges: .bottom))
    }

    /// head...tail form (U+2026), the Polkadot short-address style the desktop
    /// footer uses.
    static func shorten(_ pubkey: String) -> String {
        guard pubkey.count > 13 else { return pubkey }
        return "\(pubkey.prefix(6))\u{2026}\(pubkey.suffix(6))"
    }
}
