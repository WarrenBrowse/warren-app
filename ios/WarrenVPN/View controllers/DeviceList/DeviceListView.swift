//
//  DeviceListView.swift
//  WarrenVPN
//
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import SwiftUI
import WarrenRustRuntime

/// The account's devices as one card, current device first, then the
/// older-client row when the registry saw one.
struct DeviceListView: View {
    let devices: [WarrenAccountDevice]
    let olderClientSeenOn: WarrenRegistryDay?
    let today: WarrenRegistryDay
    let busy: Set<String>
    var onRename: ((WarrenAccountDevice) -> Void)?
    var onAction: ((WarrenAccountDevice) -> Void)?

    var body: some View {
        VStack(spacing: 0) {
            ForEach(Array(devices.enumerated()), id: \.element.id) { index, device in
                if index > 0 {
                    RowSeparator()
                }
                DeviceRowView(
                    device: device,
                    today: today,
                    isBusy: busy.contains(device.id),
                    onRename: { onRename?(device) },
                    onAction: { onAction?(device) }
                )
            }
            if let olderClientSeenOn {
                if !devices.isEmpty {
                    RowSeparator()
                }
                OlderClientRowView(lastSeenOn: olderClientSeenOn)
            }
        }
        .background(Color(UIColor.primaryColor))
        .clipShape(RoundedRectangle(cornerRadius: 16))
    }
}

#Preview {
    DeviceListView(
        devices: MockDeviceManaging.sample.devices,
        olderClientSeenOn: MockDeviceManaging.sample.olderClientSeenOn,
        today: WarrenRegistryDay(utc: Date()),
        busy: []
    )
    .padding()
    .background(Color.warrenBackground)
}
