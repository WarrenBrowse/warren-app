//
//  IncludeAllNetworksCheckboxToggleStyle.swift
//  MullvadVPN
//
//  Created by Jon Petersson on 2026-01-19.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import SwiftUI

struct IncludeAllNetworksCheckboxToggleStyle: ToggleStyle {
    func makeBody(configuration: Configuration) -> some View {
        Button(
            action: {
                if !configuration.isOn {
                    configuration.isOn = true
                }
            },
            label: {
                HStack {
                    (configuration.isOn
                        ? Image.warrenIconTick
                        : Image(uiImage: UIImage.checkboxUnselected))
                        .padding(8)
                    configuration.label
                        .multilineTextAlignment(.leading)
                        .font(.warrenTiny)
                    Spacer()
                }
            }
        )
        .buttonStyle(PlainButtonStyle())
    }
}
