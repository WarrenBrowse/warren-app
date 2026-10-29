//
//  DeviceManagementView.swift
//  WarrenVPN
//
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import SwiftUI
import WarrenRustRuntime

/// The "Devices" screen (warren-core doc 115 section 7.1): the account's
/// devices, renamed by tapping a name, another device disconnected after a
/// confirmation, and this device logged out through the logout confirmation
/// the host shows.
struct DeviceManagementView: View {
    @ObservedObject var viewModel: DeviceManagementViewModel
    /// "Log out" on this device's row: the host asks with the account's
    /// logout confirmation, then calls `logOutThisDevice`.
    var onLogOutThisDevice: ((WarrenAccountDevice) -> Void)?

    @State private var alert: MullvadAlert?
    @State private var renameAlert: MullvadInputAlert?

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                Text("Devices signed in to this account. Disconnecting one signs it out: it needs the recovery phrase to sign in again.")
                    .font(.warrenTiny)
                    .foregroundStyle(Color.warrenTextSecondary)
                    .fixedSize(horizontal: false, vertical: true)
                content
            }
            .padding(16)
        }
        .background(Color.warrenBackground)
        .warrenAlert(item: $alert)
        .warrenInputAlert(item: $renameAlert)
        .task {
            await viewModel.fetchDevices()
        }
        .onChange(of: viewModel.failure) { _, failure in
            guard let failure else { return }
            viewModel.failure = nil
            alert = MullvadAlert(
                type: .error,
                customView: AnyView(
                    Text(verbatim: failure.message)
                        .font(.warrenSmall)
                        .foregroundStyle(Color.warrenTextSecondary)
                        .frame(maxWidth: .infinity, alignment: .leading)
                ),
                actions: [.init(type: .default, title: "OK", handler: { alert = nil })]
            )
        }
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier(.deviceManagementView)
    }

    @ViewBuilder
    private var content: some View {
        switch viewModel.loadState {
        case .loading:
            HStack {
                Spacer()
                ProgressView()
                    .progressViewStyle(MullvadProgressViewStyle())
                Spacer()
            }
            .padding(.top, 32)
        case .failed:
            VStack(spacing: 16) {
                Text("Could not load your devices. Try again.")
                    .font(.warrenSmall)
                    .foregroundStyle(Color.warrenTextPrimary)
                    .multilineTextAlignment(.center)
                MainButton(text: "Try again", style: .default) {
                    Task { await viewModel.fetchDevices() }
                }
            }
            .frame(maxWidth: .infinity)
            .padding(.top, 32)
        case .loaded:
            DeviceListView(
                devices: viewModel.devices,
                olderClientSeenOn: viewModel.olderClientSeenOn,
                today: viewModel.todayInRegistry,
                busy: viewModel.busy,
                onRename: askRename,
                onAction: { device in
                    if device.thisDevice {
                        onLogOutThisDevice?(device)
                    } else {
                        askDisconnect(device)
                    }
                }
            )
        }
    }

    private func askDisconnect(_ device: WarrenAccountDevice) {
        alert = Self.disconnectConfirmation(
            for: device,
            onDisconnect: {
                alert = nil
                await viewModel.disconnect(device)
            },
            onCancel: { alert = nil }
        )
    }

    private func askRename(_ device: WarrenAccountDevice) {
        renameAlert = Self.renameAlert(for: device) { name in
            renameAlert = nil
            await viewModel.rename(device, to: name)
        }
    }

    /// "Disconnect <name>?", what disconnecting another device means, and the
    /// two buttons.
    static func disconnectConfirmation(
        for device: WarrenAccountDevice,
        onDisconnect: @escaping () async -> Void,
        onCancel: @escaping () async -> Void
    ) -> MullvadAlert {
        let name = DeviceListPresentation.displayedName(device.name)
        var title = AttributedString(localized: "Disconnect \(name)?")
        title.font = .warrenSmallSemiBold
        title.foregroundColor = .warrenTextPrimary
        return MullvadAlert(
            type: .warning,
            customView: AnyView(
                VStack(alignment: .leading, spacing: 8) {
                    Text(title)
                    Text(
                        "It will be signed out within 10 minutes and will need the recovery phrase to sign in again. For a lost or stolen device, move to a new account instead: a device that keeps the recovery phrase can sign in again."
                    )
                    .font(.warrenSmall)
                    .foregroundStyle(Color.warrenTextSecondary)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            ),
            actions: [
                .init(
                    type: .danger,
                    title: LocalizedStringKey(DeviceListCopy.disconnectKey),
                    identifier: .logOutDeviceConfirmButton,
                    handler: onDisconnect
                ),
                .init(
                    type: .default,
                    title: "Cancel",
                    identifier: .logOutDeviceCancelButton,
                    handler: onCancel
                ),
            ]
        )
    }

    /// "Rename device", the field opening on the name and taking at most the
    /// 40 characters the registry keeps.
    static func renameAlert(
        for device: WarrenAccountDevice,
        onSave: @escaping (String) async -> Void
    ) -> MullvadInputAlert {
        MullvadInputAlert(
            title: "Rename device",
            placeholder: "",
            action: .init(type: .default, title: "Save", identifier: nil, handler: onSave),
            validate: DeviceListPresentation.isValidName,
            dismissButtonTitle: "Cancel",
            initialText: DeviceListPresentation.displayedName(device.name),
            maxLength: DeviceListPresentation.nameMaxCharacters
        )
    }
}

#Preview {
    NavigationView {
        DeviceManagementView(viewModel: DeviceManagementViewModel(deviceManaging: MockDeviceManaging()))
            .navigationTitle("Devices")
    }
}
