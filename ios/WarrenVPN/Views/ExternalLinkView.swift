//
//  ExternalLinkView.swift
//  MullvadVPN
//
//  Created by Jon Petersson on 2026-01-22.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import SwiftUI

struct ExternalLinkView: View {
    let url: URL
    let label: String
    let font: Font
    let color: Color

    init(url: URL, label: String, font: Font, color: Color = .white) {
        self.url = url
        self.label = label
        self.font = font
        self.color = color
    }

    var body: some View {
        HStack(alignment: .center, spacing: 2) {
            Link(label, destination: url)
                .font(font)
                .underline()
            Image(.iconExtlink)
                .renderingMode(.original)
        }
        .tint(.white)
    }
}

#Preview {
    ExternalLinkView(
        url: URL(string: "https://www.warrenbrowse.com")!,
        label: NSLocalizedString("Warren website", comment: ""),
        font: .mullvadTiny,
        color: Color.red
    )
    .background(Color.mullvadBackground)
}
