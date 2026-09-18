//
//  SplitMainButton.swift
//  MullvadVPN
//
//  Created by Jon Petersson on 2024-12-05.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import SwiftUI

struct SplitMainButton: View {
    var text: LocalizedStringKey
    /// The side button's glyph as an asset. Leave nil and set `systemImage`
    /// for a symbol the asset catalog does not carry.
    var image: ImageResource?
    var systemImage: String?
    var style: MainButtonStyle.Style
    var accessibilityId: AccessibilityIdentifier?
    var secondaryAccessibilityId: AccessibilityIdentifier?
    var secondaryAccessibilityLabel: LocalizedStringKey?
    var secondaryAccessibilityHint: LocalizedStringKey?
    /// The side button alone can be unavailable while the label still works,
    /// as on Android where the shuffle is disabled until the relay list has an
    /// active exit to pick from.
    var secondaryEnabled: Bool = true

    @State private var secondaryButtonSize: CGSize = .zero
    @State private var primaryButtonSize: CGSize = .zero

    var primaryAction: () -> Void
    var secondaryAction: () -> Void

    var body: some View {
        HStack(spacing: 1) {
            Button(
                action: primaryAction,
                label: {
                    HStack {
                        Spacer()
                        Text(text)
                        Spacer()
                    }
                    .padding(.leading, secondaryButtonSize.width)
                    .sizeOfView { primaryButtonSize = $0 }
                }
            )
            .ifLet(accessibilityId) { view, value in
                view.accessibilityIdentifier(value.asString)
            }

            Button(
                action: secondaryAction,
                label: {
                    secondaryGlyph
                        .resizable()
                        .scaledToFit()
                        .padding(10)
                        .frame(
                            width: min(max(primaryButtonSize.height, 44), 60), height: max(primaryButtonSize.height, 44)
                        )
                        .sizeOfView { secondaryButtonSize = $0 }
                }
            )
            .disabled(!secondaryEnabled)
            .ifLet(secondaryAccessibilityLabel) { view, label in
                view.accessibilityLabel(label)
            }
            .ifLet(secondaryAccessibilityHint) { view, hint in
                view.accessibilityHint(hint)
            }
            .ifLet(secondaryAccessibilityId) { view, value in
                view.accessibilityIdentifier(value.asString)
            }
        }
        .buttonStyle(MainButtonStyle(style))
        .cornerRadius(UIMetrics.MainButton.cornerRadius)
    }

    /// Concrete `Image` rather than `some View`: the caller still needs
    /// `.resizable()`, which only `Image` carries.
    private var secondaryGlyph: Image {
        if let image {
            return Image(image)
        }
        if let systemImage {
            return Image(systemName: systemImage)
        }
        return Image(.iconReload)
    }
}

#Preview {
    SplitMainButton(
        text: "Select location",
        image: .iconReload,
        style: .default,
        primaryAction: {
            print("Tapped primary")
        },
        secondaryAction: {
            print("Tapped secondary")
        }
    )
}
