//
//  ChipViewModelProtocol.swift
//  MullvadVPN
//
//  Created by Mojgan on 2024-12-05.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import SwiftUI

protocol ChipViewModelProtocol: ObservableObject {
    var chips: [ChipModel] { get }
    func onPressed(item: ChipModel)
}

class MockFeatureIndicatorsViewModel: ChipViewModelProtocol {
    func onPressed(item: ChipModel) {}

    @Published var chips: [ChipModel] = [
        ChipModel(id: .daita, name: "DAITA"),
        ChipModel(id: .obfuscation, name: "Obfuscation"),
        ChipModel(id: .quantumResistance, name: "Quantum resistance"),
        ChipModel(id: .multihop, name: "Multihop"),
        ChipModel(id: .dns, name: "DNS content blockers"),
        ChipModel(id: .dns, name: "Custom DNS"),
        ChipModel(id: .ipOverrides, name: "Server IP override"),
        ChipModel(id: .includeAllNetworks, name: "Force all apps"),
    ]
}
