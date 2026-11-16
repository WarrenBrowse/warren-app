//
//  HomeView.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  The home screen (desktop `MainView`): the scenery hero over the panel
//  (status, controls, chips) and the account footer with the beta mark, the
//  screen shared between them by `HomeLayout`. The panel and the footer keep a
//  centred column; the hero spans the full width, or the leading half beside
//  the panel on a short screen wider than tall. The floating header is the
//  root container's.
//

import SwiftUI

/// The account line at the foot of the home: the shortened public key with its
/// copy button, and the time left.
final class HomeFooterModel: ObservableObject {
    @Published var pubkey: String?
    @Published var expiry: Date?
}

/// Whether the connection details are open under the exit row.
final class HomeDetailsModel: ObservableObject {
    @Published var isOpen = false
}

/// The status bar the hero draws under. SwiftUI only sees the root container's
/// safe area, which also holds the floating header, so the window's is handed in.
final class HomeHeroModel: ObservableObject {
    @Published var safeTop: CGFloat = 0
}

struct HomeView: View {
    @ObservedObject var connectionViewModel: ConnectionViewViewModel
    @ObservedObject var indicatorsViewModel: FeatureIndicatorsViewModel
    @ObservedObject var footerModel: HomeFooterModel
    @ObservedObject var details: HomeDetailsModel
    @ObservedObject var hero: HomeHeroModel
    let scenery: SceneryViewController
    let action: (ConnectionViewViewModel.TunnelAction) -> Void
    @Environment(\.colorScheme) private var colorScheme
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var panelHeight: CGFloat = 0
    @State private var footerHeight: CGFloat = 0
    /// What the open details add to the panel, taken out again to know the hero they close to.
    @State private var detailsHeight: CGFloat = 0

    /// The details show only while the tunnel is up, so a stale open state never reopens them.
    private var detailsShown: Bool {
        details.isOpen && connectionViewModel.tunnelIsConnected
    }

    var body: some View {
        let palette = ConnectSurfacePalette(colorScheme: colorScheme)

        GeometryReader { proxy in
            let layout = HomeLayout.resolve(
                width: proxy.size.width,
                height: proxy.size.height,
                safeTop: hero.safeTop,
                panelHeight: panelHeight - (detailsShown ? detailsHeight : 0),
                detailsHeight: detailsShown ? detailsHeight : 0,
                footerHeight: footerHeight
            )
            if layout.split {
                split(layout, size: proxy.size)
            } else {
                stacked(layout, size: proxy.size)
            }
        }
        .ignoresSafeArea(.container, edges: .top)
        .background(palette.color(.homePage).ignoresSafeArea())
        .animation(reduceMotion ? nil : Self.resize, value: detailsShown)
        .onChange(of: connectionViewModel.tunnelStatus.state) { details.isOpen = false }
        .accessibilityAction(.escape) { details.isOpen = false }
    }

    /// The hero over the panel, the footer pinned under both.
    private func stacked(_ layout: HomeLayout, size: CGSize) -> some View {
        VStack(spacing: 0) {
            ScrollView(.vertical, showsIndicators: false) {
                VStack(spacing: 0) {
                    heroView(layout)
                        .frame(height: layout.heroHeight)

                    measuredPanel
                        // The status block rides up over the hero's fade.
                        .padding(.top, -HomeMetrics.Panel.overlap)

                    // The room the mockup leaves between the chips and the footer.
                    Spacer(minLength: 0)
                }
                .frame(minHeight: max(0, size.height - footerHeight), alignment: .top)
            }
            .scrollDisabled(!layout.scrolls)
            .scrollBounceBehavior(.basedOnSize)
            .ignoresSafeArea(.container, edges: .top)

            footer
        }
    }

    /// The hero on the leading half at full height, the panel scrolling on the other half above
    /// the footer.
    private func split(_ layout: HomeLayout, size: CGSize) -> some View {
        HStack(spacing: 0) {
            heroView(layout)
                .frame(width: layout.heroWidth, height: size.height)

            VStack(spacing: 0) {
                ScrollView(.vertical, showsIndicators: false) {
                    measuredPanel
                        .padding(.top, hero.safeTop + HomeMetrics.Hero.headerInset)
                }
                .scrollDisabled(!layout.scrolls)
                .scrollBounceBehavior(.basedOnSize)
                .ignoresSafeArea(.container, edges: .top)

                footer
            }
        }
    }

    private func heroView(_ layout: HomeLayout) -> some View {
        HomeHeroView(
            controller: scenery,
            height: layout.heroHeight,
            openHeight: layout.fullHeight,
            compact: layout.compactFraming,
            besidePanel: layout.split,
            animated: !reduceMotion
        )
    }

    private var measuredPanel: some View {
        panel
            .frame(maxWidth: HomeMetrics.maxColumnWidth)
            .frame(maxWidth: .infinity)
            .fixedSize(horizontal: false, vertical: true)
            .sizeOfView { panelHeight = $0.height }
    }

    private var footer: some View {
        HomeFooterView(
            model: footerModel,
            beta: connectionViewModel.productBadge.map {
                HomeFooterBeta.label(badge: $0, networkInfo: connectionViewModel.networkInfo)
            },
            betaHint: connectionViewModel.productBannerLine,
            onBeta: { action(.explainNetwork) }
        )
        .sizeOfView { footerHeight = $0.height }
    }

    /// The hero's resize when the details open or close, desktop's `cubic-bezier(0.3, 0.7, 0.2, 1)`.
    static let resize = Animation.timingCurve(
        SceneryFraming.compactTransitionCurve.0.x, SceneryFraming.compactTransitionCurve.0.y,
        SceneryFraming.compactTransitionCurve.1.x, SceneryFraming.compactTransitionCurve.1.y,
        duration: TimeInterval(SceneryFraming.compactTransitionMs / 1000)
    )

    private var panel: some View {
        VStack(alignment: .leading, spacing: 0) {
            HomeStatusView(viewModel: connectionViewModel, detailsOpen: $details.isOpen) {
                detailsHeight = $0 + HomeMetrics.Details.marginTop + HomeMetrics.Panel.gap
            }
            HomeControlsView(viewModel: connectionViewModel, action: action)
            HomeChipsView(viewModel: indicatorsViewModel)
                .padding(.top, HomeMetrics.Chips.marginTop)
        }
        .padding(.bottom, 12)
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier(AccessibilityIdentifier.connectionView.asString)
    }
}

/// The scenery hero, a UIKit controller laid out in the frame SwiftUI gives it and framed for the
/// height the layout is heading to.
struct HomeHeroView: UIViewControllerRepresentable {
    let controller: SceneryViewController
    let height: CGFloat
    let openHeight: CGFloat
    let compact: Bool
    let besidePanel: Bool
    let animated: Bool

    func makeUIViewController(context: Context) -> SceneryViewController { controller }

    func updateUIViewController(_ uiViewController: SceneryViewController, context: Context) {
        uiViewController.fadesPanelEdge = besidePanel
        uiViewController.setHero(height: height, openHeight: openHeight, compact: compact, animated: animated)
    }
}

// MARK: - Status

struct HomeStatusView: View {
    @ObservedObject var viewModel: ConnectionViewViewModel
    @Binding var detailsOpen: Bool
    /// The height of the open details card, as it lays out.
    var onDetailsHeight: (CGFloat) -> Void = { _ in }
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let palette = ConnectSurfacePalette(colorScheme: colorScheme)
        let status = viewModel.homeStatus
        let phase = viewModel.connectionPhase
        let connected = viewModel.tunnelIsConnected

        VStack(alignment: .leading, spacing: HomeMetrics.Panel.gap) {
            HomePill(phase: phase, label: status.pill, palette: palette)

            Text(verbatim: status.title)
                .font(.instrumentSans(size: HomeMetrics.Title.textSize, weight: .semibold, relativeTo: .title))
                .tracking(HomeMetrics.Title.letterSpacingEm * HomeMetrics.Title.textSize)
                .foregroundStyle(palette.color(.homeText))
                .lineLimit(1)
                .truncationMode(.tail)
                .padding(.top, HomeMetrics.Title.marginTop)
                .accessibilityIdentifier(viewModel.accessibilityIdForSecureLabel.asString)
                .accessibilityLabel(viewModel.localizedAccessibilityLabelForSecureLabel)
                .accessibilityAddTraits(.isHeader)

            if connected {
                HomeExitRow(status: status, viewModel: viewModel, isOpen: detailsOpen) {
                    detailsOpen.toggle()
                }
                if detailsOpen {
                    ConnectionDetailsView(viewModel: viewModel)
                        .sizeOfView { onDetailsHeight($0.height) }
                        .padding(.top, HomeMetrics.Details.marginTop)
                        .transition(.opacity)
                }
            } else {
                subtitle(status.subtitle, palette: palette)
            }
        }
        .padding(.horizontal, HomeMetrics.Panel.paddingHorizontal)
    }

    /// The plain line under the title: one line high even when empty, so the controls below keep
    /// their place across every state.
    private func subtitle(_ text: String, palette: ConnectSurfacePalette) -> some View {
        let subtitle = HomeMetrics.Subtitle.self
        return Text(verbatim: text)
            .font(.instrumentSans(size: subtitle.textSize, relativeTo: .footnote))
            .foregroundStyle(palette.color(.homeTextSecondary))
            .lineLimit(1)
            .frame(maxWidth: .infinity, minHeight: subtitle.lineHeight, alignment: .leading)
            .padding(.vertical, subtitle.paddingVertical)
            .padding(.top, subtitle.marginTop)
            .accessibilityIdentifier("connectionStatusSubtitle")
            .accessibilityHidden(text.isEmpty)
    }
}

/// The protected exit line, one button that opens the details under it (desktop `StyledExitRow`):
/// the exit host in the mono face and a thin chevron at the far end that turns over when open.
/// Its pressed well reaches past the text on both sides, and its touch target is taller than the
/// row it draws.
struct HomeExitRow: View {
    let status: HomeStatus
    @ObservedObject var viewModel: ConnectionViewViewModel
    let isOpen: Bool
    let toggle: () -> Void
    @Environment(\.colorScheme) private var colorScheme
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        let palette = ConnectSurfacePalette(colorScheme: colorScheme)
        let row = HomeMetrics.ExitRow.self
        Button(action: toggle) {
            HStack(spacing: row.gap) {
                if !status.subtitle.isEmpty {
                    Text(verbatim: status.subtitle)
                        .font(
                            status.subtitleIsHostname
                                ? .jetBrainsMono(size: row.textSize, relativeTo: .footnote)
                                : .instrumentSans(size: HomeMetrics.Subtitle.textSize, relativeTo: .footnote)
                        )
                        .foregroundStyle(palette.color(status.subtitleIsHostname ? .homeTextMono : .homeTextSecondary))
                        .lineLimit(1)
                        .truncationMode(.middle)
                        // A host name reads left to right in every language.
                        .environment(\.layoutDirection, status.subtitleIsHostname ? .leftToRight : layoutDirection)
                        .accessibilityIdentifier(AccessibilityIdentifier.connectionPanelServerLabel.asString)
                        .accessibilityLabel(Text(verbatim: viewModel.accessibilityLabelForServer ?? status.subtitle))
                }
                Spacer(minLength: 0)
                HomeExpandMoreShape()
                    .stroke(
                        palette.color(.homeTextMono),
                        style: StrokeStyle(lineWidth: 1.5 * row.chevron / 24, lineCap: .round, lineJoin: .round)
                    )
                    .frame(width: row.chevron, height: row.chevron)
                    .rotationEffect(.degrees(isOpen ? 180 : 0))
                    .accessibilityElement()
                    .accessibilityAddTraits(.isButton)
                    .accessibilityLabel(
                        isOpen
                            ? LocalizedStringKey("Collapse connection details")
                            : LocalizedStringKey("Expand connection details")
                    )
                    .accessibilityAction { toggle() }
                    .animation(
                        reduceMotion ? nil : .timingCurve(0.25, 0.1, 0.25, 1, duration: row.chevronMs / 1000),
                        value: isOpen
                    )
            }
        }
        .buttonStyle(HomeExitRowButtonStyle())
        .padding(.top, row.marginTop)
        .padding(.horizontal, -row.outset)
        // The host stays a text of its own and the chevron the control VoiceOver reaches, as before
        // the whole row became the button.
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier(AccessibilityIdentifier.relayStatusCollapseButton.asString)
    }

    @Environment(\.layoutDirection) private var layoutDirection
}

/// The exit row's well: the details surface under a press, rounded, with the touch target grown
/// to 44 points around the 28 point row without moving anything.
struct HomeExitRowButtonStyle: ButtonStyle {
    @Environment(\.colorScheme) private var colorScheme

    func makeBody(configuration: Configuration) -> some View {
        let palette = ConnectSurfacePalette(colorScheme: colorScheme)
        let row = HomeMetrics.ExitRow.self
        let shape = RoundedRectangle(cornerRadius: row.radius, style: .continuous)
        let grow = max(0, (44 - (HomeMetrics.ExitRow.chevron + 2 * row.paddingVertical)) / 2)
        configuration.label
            .padding(.vertical, row.paddingVertical)
            .padding(.horizontal, row.outset)
            .background(shape.fill(configuration.isPressed ? palette.color(.homeDetails) : .clear))
            .padding(.vertical, grow)
            .contentShape(Rectangle())
            .padding(.vertical, -grow)
    }
}

/// The mockup's thin expand-more chevron, `M7 10l5 5 5-5` on a 24 grid.
struct HomeExpandMoreShape: Shape {
    func path(in rect: CGRect) -> Path {
        let unit = rect.width / 24
        var path = Path()
        path.move(to: CGPoint(x: rect.minX + 7 * unit, y: rect.minY + 10 * unit))
        path.addLine(to: CGPoint(x: rect.minX + 12 * unit, y: rect.minY + 15 * unit))
        path.addLine(to: CGPoint(x: rect.minX + 17 * unit, y: rect.minY + 10 * unit))
        return path
    }
}

/// The phase badge: the eye open while the traffic is visible, crossed out once
/// it is hidden, on the phase colour.
struct HomePill: View {
    let phase: ConnectionPhase
    let label: String
    let palette: ConnectSurfacePalette

    var body: some View {
        let colors = phase.homePillTokens
        let pill = HomeMetrics.Pill.self
        HStack(spacing: pill.gap) {
            Image(systemName: phase.showsCrossedEye ? "eye.slash" : "eye")
                .font(.system(size: pill.icon - 3, weight: .semibold))
                .frame(width: pill.icon, height: pill.icon)
            Text(verbatim: label)
                .font(.instrumentSans(size: pill.textSize, weight: .bold, relativeTo: .footnote))
                .lineLimit(1)
        }
        .foregroundStyle(palette.color(colors.text))
        .padding(.vertical, pill.paddingVertical)
        .padding(.leading, pill.paddingStart)
        .padding(.trailing, pill.paddingEnd)
        .background(
            Capsule()
                .fill(palette.color(colors.background))
                .shadow(color: palette.color(colors.background).opacity(0.6), radius: 7, x: 0, y: 5)
        )
        .animation(.easeOut(duration: 0.3), value: phase)
        .accessibilityIdentifier("connectionStatusEye")
    }
}

extension ConnectionPhase {
    /// The pill's fill and text, desktop's `pillColors`.
    var homePillTokens: (background: ConnectSurfaceToken, text: ConnectSurfaceToken) {
        switch self {
        case .exposed: (.homePillExposed, .homePillExposedText)
        case .connecting, .interrupted: (.homePillPending, .homePillPendingText)
        case .protected: (.homePillProtected, .homePillProtectedText)
        case .blocked: (.homePillBlocked, .homePillBlockedText)
        }
    }
}
