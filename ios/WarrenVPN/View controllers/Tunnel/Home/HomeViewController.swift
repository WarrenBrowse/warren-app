//
//  HomeViewController.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import SwiftUI
import UIKit

/// The home screen's content: the scenery hero and the SwiftUI panel and footer
/// over the page colour. The tunnel screen embeds it and owns everything the
/// buttons do; the floating header is the root container's.
final class HomeViewController: UIViewController {
    let sceneryViewController: SceneryViewController
    private let details: HomeDetailsModel
    private let hero: HomeHeroModel
    private let hostingController: UIHostingController<HomeView>

    /// The tunnel screen's handler for the home's buttons.
    var action: ((ConnectionViewViewModel.TunnelAction) -> Void)?

    init(
        connectionViewModel: ConnectionViewViewModel,
        indicatorsViewModel: FeatureIndicatorsViewModel,
        footerModel: HomeFooterModel
    ) {
        // The closure is wired before `self` exists, so it reaches the handler
        // through a box filled in once init completes.
        let box = ActionBox()
        let scenery = SceneryViewController()
        let details = HomeDetailsModel()
        let hero = HomeHeroModel()
        sceneryViewController = scenery
        self.details = details
        self.hero = hero
        hostingController = UIHostingController(
            rootView: HomeView(
                connectionViewModel: connectionViewModel,
                indicatorsViewModel: indicatorsViewModel,
                footerModel: footerModel,
                details: details,
                hero: hero,
                scenery: scenery,
                action: { box.handler?($0) }
            )
        )
        super.init(nibName: nil, bundle: nil)
        box.handler = { [weak self] in self?.action?($0) }
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    /// The palette the whole home paints in, from the user's theme choice.
    var theme: WarrenTheme = .dark {
        didSet { overrideUserInterfaceStyle = theme.userInterfaceStyle }
    }

    override func viewDidLoad() {
        super.viewDidLoad()
        overrideUserInterfaceStyle = theme.userInterfaceStyle
        view.backgroundColor = ConnectSurfacePalette.dynamic(.homePage)

        let hostedView = hostingController.view!
        hostedView.backgroundColor = .clear
        addChild(hostingController)
        view.addConstrainedSubviews([hostedView]) {
            hostedView.pinEdgesToSuperview()
        }
        hostingController.didMove(toParent: self)
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        // The art runs under the status bar and the floating header; it keeps
        // what matters below the header, which sits under the status bar.
        let safeTop = view.window?.safeAreaInsets.top ?? 0
        sceneryViewController.safeTop = safeTop
        if hero.safeTop != safeTop {
            hero.safeTop = safeTop
        }
    }

    /// Opens the connection details under the exit row, as a tap on it does.
    func openDetails() {
        details.isOpen = true
    }

    func update(phase: ConnectionPhase, exitCountry: String?, animated: Bool) {
        sceneryViewController.update(phase: phase, exitCountry: exitCountry, animated: animated)
    }

    private final class ActionBox {
        var handler: ((ConnectionViewViewModel.TunnelAction) -> Void)?
    }
}
