//
//  DeviceRowView.swift
//  WarrenVPN
//
//  Created by pronebird on 26/07/2022.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import SwiftUI
import WarrenRustRuntime

/// One device of the account: its name (tap to rename), the "This device"
/// badge, "<Platform> · <version>", its last activity, and "Log out" on this
/// device or "Disconnect" on another one.
struct DeviceRowView: View {
    let device: WarrenAccountDevice
    let today: WarrenRegistryDay
    let isBusy: Bool
    var onRename: (() -> Void)?
    var onAction: (() -> Void)?

    @Environment(\.locale) private var locale

    var body: some View {
        HStack(alignment: .center, spacing: 12) {
            VStack(alignment: .leading, spacing: 4) {
                HStack(spacing: 8) {
                    Button {
                        onRename?()
                    } label: {
                        Text(verbatim: DeviceListPresentation.displayedName(device.name))
                            .font(.warrenSmallSemiBold)
                            .foregroundStyle(Color.warrenTextPrimary)
                            .multilineTextAlignment(.leading)
                    }
                    .buttonStyle(.plain)
                    .accessibilityHint(Text("Rename device"))
                    if device.thisDevice {
                        Text("This device")
                            .font(.warrenMini)
                            .foregroundStyle(Color.white)
                            .padding(.horizontal, 6)
                            .padding(.vertical, 2)
                            .background(Color.warrenSuccessColor)
                            .clipShape(RoundedRectangle(cornerRadius: 4))
                    }
                }
                Text(verbatim: DeviceListPresentation.platformLine(device))
                    .font(.warrenMini)
                    .foregroundStyle(Color.warrenTextSecondary)
                if let lastSeenOn = device.lastSeenOn {
                    Text(
                        verbatim: DeviceListPresentation.activityLabel(
                            lastSeenOn: lastSeenOn, today: today, locale: locale)
                    )
                    .font(.warrenMini)
                    .foregroundStyle(Color.warrenTextSecondary)
                }
            }
            Spacer(minLength: 8)
            if isBusy {
                ProgressView()
                    .progressViewStyle(MullvadProgressViewStyle())
                    .frame(width: 24, height: 24)
                    .accessibilityIdentifier(.deviceRemovalProgressView)
            } else {
                Button {
                    onAction?()
                } label: {
                    (device.thisDevice ? Text("Log out") : Text(verbatim: DeviceListCopy.disconnect))
                        .font(.warrenSmallSemiBold)
                        .foregroundStyle(Color.warrenDangerColor)
                }
                .buttonStyle(.plain)
                .accessibilityIdentifier(device.thisDevice ? .logoutButton : .deviceCellRemoveButton)
            }
        }
        .padding(16)
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier(.deviceCell)
    }
}

/// A device too old to register: it cannot be named or disconnected, only
/// pointed at, since a registered device already took its place.
struct OlderClientRowView: View {
    let lastSeenOn: WarrenRegistryDay

    @Environment(\.locale) private var locale

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text("An older version of Warren")
                .font(.warrenSmallSemiBold)
                .foregroundStyle(Color.warrenTextPrimary)
            Text(
                "A device of this account runs a version of Warren older than 1.1.36. It no longer blocks your other devices. Update or uninstall it."
            )
            .font(.warrenMini)
            .foregroundStyle(Color.warrenTextSecondary)
            .fixedSize(horizontal: false, vertical: true)
            Text(verbatim: DeviceListPresentation.olderClientSeenLabel(lastSeenOn, locale: locale))
                .font(.warrenMini)
                .foregroundStyle(Color.warrenTextSecondary)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(16)
    }
}
