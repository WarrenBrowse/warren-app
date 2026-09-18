//
//  WarrenForumActivityCoordinator.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import Routing
import SwiftUI
import UIKit
import WarrenRustRuntime

/// The forum activity panel, presented from the header bell.
final class WarrenForumActivityCoordinator: Coordinator, Presentable {
    private let navigationController: UINavigationController
    private let viewModel: WarrenForumActivityViewModel

    var didFinish: ((WarrenForumActivityCoordinator) -> Void)?

    var presentedViewController: UIViewController {
        navigationController
    }

    init(
        navigationController: UINavigationController,
        viewModel: WarrenForumActivityViewModel
    ) {
        self.navigationController = navigationController
        self.viewModel = viewModel
    }

    func start(animated: Bool) {
        let view = WarrenForumActivityView(
            viewModel: viewModel,
            handle: WarrenForumIdentityStore.load()?.handle,
            openURL: { url in
                UIApplication.shared.open(url)
            }
        )
        let controller = UIHostingController(rootView: view)
        controller.navigationItem.title = NSLocalizedString(
            "Forum",
            tableName: "Settings",
            comment: ""
        )
        controller.navigationItem.largeTitleDisplayMode = .always
        navigationController.navigationBar.prefersLargeTitles = true

        let done = UIBarButtonItem(
            title: NSLocalizedString("Done", comment: ""),
            primaryAction: UIAction { [weak self] _ in
                guard let self else { return }
                didFinish?(self)
            }
        )
        done.style = .done
        controller.navigationItem.rightBarButtonItem = done

        navigationController.pushViewController(controller, animated: animated)
    }
}
