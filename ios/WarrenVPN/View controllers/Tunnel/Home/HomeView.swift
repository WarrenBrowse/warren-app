//
//  HomeView.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  The home screen (desktop `MainView`): the scenery hero keeps the mockup's
//  proportion of the panel column (300 high on 360) plus the status bar, and
//  the panel below it (status, controls, chips, then the room the mockup
//  leaves, the beta line and the account footer) takes the rest. The floating
//  header is the root container's.
//

import SwiftUI

/// The account line at the foot of the home: the shortened public key with its
/// copy button, and the time left.
final class HomeFooterModel: ObservableObject {
    @Published var pubkey: String?
    @Published var expiry: Date?
}

/// Whether the connection details are open over the hero.
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

    var body: some View {
        let palette = ConnectSurfacePalette(colorScheme: colorScheme)

        GeometryReader { proxy in
            VStack(spacing: 0) {
                HomeHeroView(controller: scenery)
                    // The column width drives the height, so a tablet keeps the
                    // phone's proportions. On a screen too short for the panel
                    // the hero gives way first.
                    .frame(
                        maxWidth: .infinity,
                        maxHeight: SceneryFraming.heroHeight(
                            columnWidth: HomeMetrics.columnWidth(forWidth: proxy.size.width),
                            safeTop: hero.safeTop
                        )
                    )
                    .layoutPriority(1)
                    .overlay(alignment: .bottom) {
                        if details.isOpen && connectionViewModel.showsConnectionDetails {
                            HomeDetailsSheet(viewModel: connectionViewModel)
                                .transition(.opacity.combined(with: .offset(y: 12)))
                        }
                    }

                panel
                    .frame(maxWidth: HomeMetrics.maxColumnWidth)
                    .frame(maxWidth: .infinity)
                    // The status block rides up over the hero's fade.
                    .padding(.top, -HomeMetrics.Panel.overlap)
                    .fixedSize(horizontal: false, vertical: true)

                // The room the mockup leaves between the chips and the footer.
                Spacer(minLength: 0)

                if let badge = connectionViewModel.productBadge {
                    HomeBetaLine(badge: badge, line: connectionViewModel.productBannerLine) {
                        action(.explainNetwork)
                    }
                    .padding(.vertical, 10)
                    .frame(maxWidth: HomeMetrics.maxColumnWidth)
                }

                HomeFooterView(model: footerModel)
            }
        }
        .ignoresSafeArea(.container, edges: .top)
        .background(palette.color(.homePage).ignoresSafeArea())
        .animation(.easeOut(duration: 0.2), value: details.isOpen)
        .onChange(of: connectionViewModel.tunnelStatus.state) { details.isOpen = false }
    }

    private var panel: some View {
        VStack(alignment: .leading, spacing: 0) {
            HomeStatusView(viewModel: connectionViewModel, detailsOpen: $details.isOpen)
            HomeControlsView(viewModel: connectionViewModel, action: action)
            HomeChipsView(viewModel: indicatorsViewModel)
                .padding(.top, HomeMetrics.Chips.marginTop)
        }
        .padding(.bottom, 12)
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier(AccessibilityIdentifier.connectionView.asString)
    }
}

/// The scenery hero, a UIKit controller laid out in the frame SwiftUI gives it.
struct HomeHeroView: UIViewControllerRepresentable {
    let controller: SceneryViewController

    func makeUIViewController(context: Context) -> SceneryViewController { controller }

    func updateUIViewController(_ uiViewController: SceneryViewController, context: Context) {}
}

// MARK: - Status

struct HomeStatusView: View {
    @ObservedObject var viewModel: ConnectionViewViewModel
    @Binding var detailsOpen: Bool
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let palette = ConnectSurfacePalette(colorScheme: colorScheme)
        let status = viewModel.homeStatus
        let phase = viewModel.connectionPhase

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

            subtitleRow(status: status, palette: palette)
        }
        .padding(.horizontal, HomeMetrics.Panel.paddingHorizontal)
    }

    /// Fixed in height whether or not the details button is there, so the hero
    /// above keeps its frame across every state.
    private func subtitleRow(status: HomeStatus, palette: ConnectSurfacePalette) -> some View {
        HStack(spacing: 8) {
            if !status.subtitle.isEmpty {
                if status.subtitleIsHostname {
                    Text(verbatim: status.subtitle)
                        .font(.jetBrainsMono(size: HomeMetrics.Subtitle.monoSize, relativeTo: .footnote))
                        .foregroundStyle(palette.color(.homeTextMono))
                        .lineLimit(1)
                        .truncationMode(.middle)
                        // A host name reads left to right in every language.
                        .environment(\.layoutDirection, .leftToRight)
                        .accessibilityIdentifier(AccessibilityIdentifier.connectionPanelServerLabel.asString)
                        .accessibilityLabel(viewModel.accessibilityLabelForServer ?? status.subtitle)
                } else {
                    Text(verbatim: status.subtitle)
                        .font(.instrumentSans(size: HomeMetrics.Subtitle.textSize, relativeTo: .footnote))
                        .foregroundStyle(palette.color(.homeTextSecondary))
                        .lineLimit(1)
                        .accessibilityIdentifier("connectionStatusSubtitle")
                }
            }
            Spacer(minLength: 0)
            if viewModel.showsConnectionDetails {
                Image(systemName: detailsOpen ? "chevron.down" : "chevron.up")
                    .font(.system(size: 14, weight: .semibold))
                    .foregroundStyle(palette.color(.homeTextSecondary))
                    .frame(width: 24, height: 18)
                    .accessibilityAddTraits(.isButton)
                    .accessibilityLabel(
                        detailsOpen
                            ? LocalizedStringKey("Collapse connection details")
                            : LocalizedStringKey("Expand connection details")
                    )
            }
        }
        .frame(minHeight: 18)
        .contentShape(Rectangle())
        .onTapGesture {
            guard viewModel.showsConnectionDetails else { return }
            detailsOpen.toggle()
        }
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier(AccessibilityIdentifier.relayStatusCollapseButton.asString)
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

// MARK: - Details

/// The connection details open over the hero rather than pushing the controls:
/// the art is what can give way.
struct HomeDetailsSheet: View {
    @ObservedObject var viewModel: ConnectionViewViewModel
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let palette = ConnectSurfacePalette(colorScheme: colorScheme)
        ScrollView {
            VStack(alignment: .leading, spacing: 8) {
                ConnectionDetailsView(viewModel: viewModel)
                // Always-on HTTP/3 mimicry, shown while the tunnel is secured so
                // users know their traffic looks like regular HTTPS.
                if viewModel.tunnelStatus.state.isSecured {
                    WarrenObfuscationIndicatorView(palette: palette)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal, HomeMetrics.Panel.paddingHorizontal)
            .padding(.top, 16)
            // The status block rides up over the sheet's foot, as it does over
            // the hero.
            .padding(.bottom, 16 + HomeMetrics.Panel.overlap)
        }
        .scrollBounceBehavior(.basedOnSize)
        // As tall as the details, up to 300 points, then it scrolls.
        .frame(maxHeight: 300)
        .fixedSize(horizontal: false, vertical: true)
        .frame(maxWidth: HomeMetrics.maxColumnWidth)
        .background(
            UnevenRoundedRectangle(topLeadingRadius: 16, topTrailingRadius: 16)
                .fill(palette.color(.homePage))
                .shadow(color: .black.opacity(0.45), radius: 12, x: 0, y: -6)
        )
    }
}
