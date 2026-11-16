//
//  HomeControlsView.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  The home's controls (desktop `HomeControls`): the location row with its
//  round picture and the square shuffle beside it, then the action button,
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
                HomeLocationThumbView(thumb: location.thumb)
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

/// The round picture of a location: a square of its landscape crowned by the
/// skyline, the flag of a country without art, the watched plain otherwise.
struct HomeLocationThumbView: View {
    let thumb: HomeLocationThumb

    var body: some View {
        let size = HomeMetrics.Controls.thumb
        Group {
            switch thumb {
            case let .landscape(imageName, topRow):
                HomeLandscapeSquare(imageName: imageName, topRow: topRow)
            case let .flag(code):
                Text(verbatim: Self.flagEmoji(countryCode: code) ?? "")
                    .font(.system(size: size * 1.45))
                    .frame(width: size, height: size)
                    .background(Color.black.opacity(0.1))
            }
        }
        .frame(width: size, height: size)
        .clipShape(Circle())
        .accessibilityHidden(true)
    }

    static func flagEmoji(countryCode: String) -> String? {
        let letterA: Unicode.Scalar = "A"
        let regionalIndicatorA: UInt32 = 0x1F1E6
        var flag = ""
        for letter in countryCode.uppercased().unicodeScalars {
            guard letter.value >= letterA.value, letter.value < letterA.value + 26,
                let scalar = Unicode.Scalar(regionalIndicatorA + letter.value - letterA.value)
            else { return nil }
            flag.unicodeScalars.append(scalar)
        }
        return flag.unicodeScalars.count == 2 ? flag : nil
    }
}

/// A full-width square of a landscape from `topRow` down, cut once and cached:
/// the thumbnail is 28 points, the master 1140 pixels wide.
private struct HomeLandscapeSquare: View {
    let imageName: String
    let topRow: CGFloat

    var body: some View {
        if let image = Self.square(imageName: imageName, topRow: topRow) {
            Image(uiImage: image)
                .resizable()
                .interpolation(.high)
        } else {
            Color.black.opacity(0.1)
        }
    }

    private static let cache = NSCache<NSString, UIImage>()

    static func square(imageName: String, topRow: CGFloat) -> UIImage? {
        let key = "\(imageName)@\(Int(topRow))" as NSString
        if let cached = cache.object(forKey: key) { return cached }
        guard let source = UIImage(named: imageName), let cgImage = source.cgImage else { return nil }
        let pixelsPerRow = CGFloat(cgImage.width) / SceneryFraming.canvasWidth
        let side = CGFloat(cgImage.width)
        guard
            let cropped = cgImage.cropping(
                to: CGRect(x: 0, y: (topRow * pixelsPerRow).rounded(), width: side, height: side))
        else { return nil }
        let thumbSide: CGFloat = 112
        let format = UIGraphicsImageRendererFormat()
        format.scale = 1
        let square = UIGraphicsImageRenderer(size: CGSize(width: thumbSide, height: thumbSide), format: format)
            .image { _ in
                UIImage(cgImage: cropped).draw(in: CGRect(x: 0, y: 0, width: thumbSide, height: thumbSide))
            }
        cache.setObject(square, forKey: key)
        return square
    }
}
