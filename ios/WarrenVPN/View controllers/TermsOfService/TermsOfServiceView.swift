//
//  TermsOfServiceView.swift
//  MullvadVPN
//
//  Created by Marco Nikic on 2025-06-05.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import SwiftUI

struct TermsOfServiceView: View {
    public var agreeToTermsAndServices: (() -> Void)?
    let padding = EdgeInsets(top: 24, leading: 16, bottom: 24, trailing: 16)
    @ScaledMetric(relativeTo: .footnote)
    var imageHeight = 20

    let termsOfService = LocalizedStringKey(
        """
        Warren keeps no activity logs and asks for no name or email.

        The privacy policy lists the data Warren does process, such as payment references kept for \
        refunds, and the data you choose to send, for example in a problem report.
        """)

    let privacyPolicyText = NSLocalizedString("privacy policy", comment: "").localizedCapitalized
    var scrollableContent: some View {
        ScrollView {
            Text(LocalizedStringKey("Before you start"))
                .font(.warrenLarge)
                .foregroundStyle(.white)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.bottom, 16)
            Text(termsOfService)
                .font(.warrenSmall)
                .foregroundStyle(Color.secondaryTextColor)
        }
        .padding(padding)
    }

    var body: some View {
        VStack(alignment: .leading) {
            scrollableContent.scrollBounceBehavior(.basedOnSize)
            HStack {
                Text(
                    LocalizedStringKey(
                        stringLiteral:
                            "[\(privacyPolicyText)](\(ApplicationConfiguration.privacyPolicyLink(for :ApplicationLanguage.currentLanguage.id)))"
                    )
                )
                .font(.warrenSmall)
                .underline(true, color: .white)
                .foregroundStyle(.white)
                .tint(.white)
                Image(uiImage: UIImage.iconExtLink)
                    .resizable()
                    .scaledToFit()
                    .frame(height: imageHeight)
                    .foregroundStyle(.white)
            }
            .padding(padding)
            MainButton(
                text: LocalizedStringKey("Agree and continue"),
                style: .default,
                action: agreeToTermsAndServices ?? {}
            )
            .accessibilityIdentifier(AccessibilityIdentifier.agreeButton.asString)
            .padding(padding)
            .background(Color(UIColor.secondaryColor))
        }
        .background(Color(UIColor.primaryColor))
    }
}

#Preview {
    TermsOfServiceView()
        .frame(maxWidth: .infinity, maxHeight: .infinity)
}
