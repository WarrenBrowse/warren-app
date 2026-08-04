//
//  WarrenBlockedUpdateView.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  Forced-update screen shown when the verified update manifest reports
//  the running version is no longer supported. It replaces the whole UI:
//  there is no dismissal and no way back, only the App Store link.
//  Mirrors the desktop `BlockedUpdateView`. The tunnel is intentionally
//  left untouched: the user stays protected while they update.
//

import Routing
import SwiftUI
import UIKit

struct WarrenBlockedUpdateView: View {
    var onOpenAppStore: (() -> Void)?

    var body: some View {
        VStack(spacing: 0) {
            ScrollView {
                VStack(alignment: .leading, spacing: 0) {
                    HStack {
                        Spacer()
                        Image.warrenIconFail
                        Spacer()
                    }

                    Text("Update required")
                        .font(.warrenLarge)
                        .foregroundStyle(Color.MullvadText.onBackgroundEmphasis100)
                        .padding(.top, 16)

                    Text(
                        "This version of Warren is no longer supported. To keep your connection protected, you must update the app to keep using it."
                    )
                    .font(.warrenSmall)
                    .foregroundStyle(Color.MullvadText.onBackground)
                    .padding(.top, 8)
                }
                .padding(.top, 24)
            }
            .scrollBounceBehavior(.automatic)
            MainButton(text: "Open the App Store", style: .success) {
                onOpenAppStore?()
            }
        }
        .padding(.horizontal, 16)
        .padding(.bottom, 16)
        .background(Color.warrenBackground)
    }
}

/// Pushes the forced-update screen onto the root navigation container.
/// Deliberately exposes no `didFinish`: the route is terminal.
final class WarrenBlockedUpdateCoordinator: Coordinator {
    private let navigationController: RootContainerViewController

    init(navigationController: RootContainerViewController) {
        self.navigationController = navigationController
    }

    func start(animated: Bool) {
        let view = WarrenBlockedUpdateView(onOpenAppStore: {
            let link = WarrenAppStoreListing.url
            if UIApplication.shared.canOpenURL(link) {
                UIApplication.shared.open(link, options: [:], completionHandler: nil)
            }
        })

        let controller = UIHostingController(rootView: view)
        navigationController.pushViewController(controller, animated: animated)
    }
}

#Preview {
    WarrenBlockedUpdateView()
}
