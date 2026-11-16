//
//  HomeControlsView.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  The home's controls (desktop `HomeControls`): the location row with the
//  country's round flag and the square shuffle beside it, then the action button,
//  whose fill says what the tap does: a solid go, an outlined stop.
//

import SwiftUI

struct HomeControlsView: View {
    @ObservedObject var viewModel: ConnectionViewViewModel
    let action: (ConnectionViewViewModel.TunnelAction) -> Void

    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let controls = HomeMetrics.Controls.self
        VStack(spacing: controls.gap) {
            HStack(spacing: controls.gap) {
                locationButton
                shuffleButton
            }
            actionButton
        }
        .disabled(viewModel.disableButtons)
        .padding(.top, controls.marginTop)
        .padding(.horizontal, HomeMetrics.Panel.paddingHorizontal)
    }

    private var locationButton: some View {
        let palette = ConnectSurfacePalette(colorScheme: colorScheme)
        let controls = HomeMetrics.Controls.self
        let location = viewModel.homeLocation
        return Button {
            action(.selectLocation)
        } label: {
            HStack(spacing: controls.rowGap) {
                if let code = location.flagCode, let flag = HomeLocationFlagView.image(countryCode: code) {
                    HomeLocationFlagView(image: flag)
                }
                Text(verbatim: location.name)
                    .font(.instrumentSans(size: controls.labelSize, weight: .medium, relativeTo: .body))
                    .foregroundStyle(palette.color(.homeText))
                    .lineLimit(1)
                    .frame(maxWidth: .infinity, alignment: .leading)
                Text(LocalizedStringKey("Change"))
                    .font(.instrumentSans(size: controls.changeSize, relativeTo: .footnote))
                    .foregroundStyle(palette.color(.homeTextSecondary))
                    .lineLimit(1)
                    .fixedSize()
            }
            .padding(.leading, controls.paddingStart)
            .padding(.trailing, controls.paddingEnd)
            .frame(maxWidth: .infinity, minHeight: controls.rowHeight)
        }
        .buttonStyle(HomeFieldButtonStyle())
        .accessibilityLabel(
            String(
                format: NSLocalizedString("Select location. Current location is %@", comment: ""),
                location.name
            )
        )
        .accessibilityIdentifier(AccessibilityIdentifier.selectLocationButton.asString)
    }

    private var shuffleButton: some View {
        let palette = ConnectSurfacePalette(colorScheme: colorScheme)
        let side = HomeMetrics.Controls.rowHeight
        return Button {
            action(.shuffleLocation)
        } label: {
            Image(systemName: "shuffle")
                .font(.system(size: 17, weight: .medium))
                .foregroundStyle(palette.color(.homeFieldIcon))
                .frame(width: side, height: side)
        }
        .buttonStyle(HomeFieldButtonStyle())
        // Disabled until the relay list has an active exit, so a tap never
        // silently does nothing before the list loads.
        .disabled(!viewModel.shuffleEnabled)
        .accessibilityLabel(LocalizedStringKey("Random location"))
        .accessibilityHint(LocalizedStringKey("Connect to a randomly selected location"))
        .accessibilityIdentifier(AccessibilityIdentifier.shuffleLocationButton.asString)
    }

    @ViewBuilder
    private var actionButton: some View {
        let style = HomeActionButtonStyle(tone: viewModel.homeActionTone)
        switch viewModel.actionButton {
        case .connect:
            Button {
                action(.connect)
            } label: {
                Text(LocalizedStringKey("Connect"))
            }
            .buttonStyle(style)
            .accessibilityIdentifier(AccessibilityIdentifier.connectButton.asString)
        case .disconnect:
            Button {
                action(.disconnect)
            } label: {
                Text(LocalizedStringKey("Disconnect"))
            }
            .buttonStyle(style)
            .accessibilityIdentifier(AccessibilityIdentifier.disconnectButton.asString)
        case .cancel:
            // The blocked state's cancel turns the kill switch off: it says
            // Disconnect, in the neutral field.
            let disconnects = viewModel.homeActionTone == .neutral
            Button {
                action(.cancel)
            } label: {
                Text(LocalizedStringKey(disconnects ? "Disconnect" : "Cancel"))
            }
            .buttonStyle(style)
            .accessibilityIdentifier(
                disconnects
                    ? AccessibilityIdentifier.disconnectButton.asString
                    : AccessibilityIdentifier.cancelButton.asString
            )
        }
    }
}

/// What the action button looks like, from what its tap does.
enum HomeActionTone: Equatable {
    case connect
    case disconnect
    case cancel
    case neutral
}

extension ConnectionViewViewModel {
    /// A solid go, an outlined stop, an amber cancel while connecting, and the
    /// neutral field while the kill switch holds the traffic (desktop
    /// `actionTones`).
    var homeActionTone: HomeActionTone {
        if connectionPhase == .blocked {
            return .neutral
        }
        return switch actionButton {
        case .connect: .connect
        case .disconnect: .disconnect
        case .cancel: .cancel
        }
    }
}

/// The location row and the shuffle: a raised field of the page.
struct HomeFieldButtonStyle: ButtonStyle {
    @Environment(\.colorScheme) private var colorScheme
    @Environment(\.isEnabled) private var isEnabled

    func makeBody(configuration: Configuration) -> some View {
        let palette = ConnectSurfacePalette(colorScheme: colorScheme)
        let shape = RoundedRectangle(cornerRadius: HomeMetrics.Controls.radius, style: .continuous)
        configuration.label
            .background(shape.fill(palette.color(configuration.isPressed ? .homeFieldPressed : .homeField)))
            .contentShape(shape)
            .opacity(isEnabled ? 1 : 0.5)
    }
}

struct HomeActionButtonStyle: ButtonStyle {
    let tone: HomeActionTone

    @Environment(\.colorScheme) private var colorScheme
    @Environment(\.isEnabled) private var isEnabled

    func makeBody(configuration: Configuration) -> some View {
        let palette = ConnectSurfacePalette(colorScheme: colorScheme)
        let controls = HomeMetrics.Controls.self
        let shape = RoundedRectangle(cornerRadius: controls.radius, style: .continuous)
        let colors = tokens(pressed: configuration.isPressed)
        configuration.label
            .font(.instrumentSans(size: controls.buttonTextSize, weight: .semibold, relativeTo: .body))
            .foregroundStyle(palette.color(colors.text))
            .lineLimit(1)
            .padding(.horizontal, 12)
            .frame(maxWidth: .infinity, minHeight: controls.buttonHeight)
            .background(shape.fill(colors.fill.map { palette.color($0) } ?? .clear))
            .overlay {
                if let line = colors.line {
                    shape.strokeBorder(palette.color(line), lineWidth: controls.outlineWidth)
                }
            }
            .contentShape(shape)
            .opacity(isEnabled ? 1 : 0.5)
    }

    private func tokens(pressed: Bool) -> (
        fill: ConnectSurfaceToken?, line: ConnectSurfaceToken?, text: ConnectSurfaceToken
    ) {
        switch tone {
        case .connect:
            (pressed ? .homeConnectPressed : .homeConnect, nil, .homeConnectText)
        case .disconnect:
            (pressed ? .homeDisconnectHover : nil, .homeDisconnectLine, .homeDisconnectText)
        case .cancel:
            (pressed ? .homeCancelHover : nil, .homeCancelLine, .homeCancelText)
        case .neutral:
            (pressed ? .homeFieldPressed : .homeField, nil, .homeText)
        }
    }
}

/// The round flag of the location's country, as the mockup's location row shows it, under a
/// hairline so a flag with a white field keeps its edge on the light theme's paper. The flags are
/// rasterised from desktop's SVGs by `ios/scripts/generate-round-flags.sh`.
struct HomeLocationFlagView: View {
    let image: UIImage
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let controls = HomeMetrics.Controls.self
        Image(uiImage: image)
            .resizable()
            .interpolation(.high)
            .frame(width: controls.thumb, height: controls.thumb)
            .clipShape(Circle())
            .overlay(
                Circle().strokeBorder(
                    ConnectSurfacePalette(colorScheme: colorScheme).color(.homeFlagLine),
                    lineWidth: controls.thumbLine)
            )
            .accessibilityHidden(true)
    }

    /// The flag of an ISO code in either case, nil for a code the catalog has no flag for.
    static func image(countryCode: String) -> UIImage? {
        UIImage(named: "Flags/\(countryCode.lowercased())")
    }
}
