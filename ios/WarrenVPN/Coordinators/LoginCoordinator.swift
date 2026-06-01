//
//  LoginCoordinator.swift
//  MullvadVPN
//
//  Created by pronebird on 27/01/2023.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import Combine
import WarrenREST
import WarrenTypes
import Operations
import Routing
import UIKit

final class LoginCoordinator: Coordinator, Presenting {
    private let tunnelManager: TunnelManager
    private let breadcrumbsProvider: BreadcrumbsProvider
    private var breadcrumbsObserver: BreadcrumbsObserver?

    private var loginController: LoginViewController?
    private var subscriptions = Set<Combine.AnyCancellable>()

    var didFinish: (@MainActor @Sendable (LoginCoordinator) -> Void)?
    var didCreateAccount: (@MainActor @Sendable () -> Void)?
    var navigateToAccessMethods: (() -> Void)?

    var preferredAccountNumberPublisher: AnyPublisher<String, Never>?
    var presentationContext: UIViewController {
        navigationController
    }

    let navigationController: RootContainerViewController

    init(
        navigationController: RootContainerViewController,
        tunnelManager: TunnelManager,
        breadcrumbsProvider: BreadcrumbsProvider
    ) {
        self.navigationController = navigationController
        self.tunnelManager = tunnelManager
        self.breadcrumbsProvider = breadcrumbsProvider
    }

    func start(animated: Bool) {
        let interactor = LoginInteractor(tunnelManager: tunnelManager)
        let loginController = LoginViewController(
            interactor: interactor,
            alertPresenter: AlertPresenter(context: self)
        )

        loginController.navigateToAccessMethods = navigateToAccessMethods

        loginController.didFinishLogin = { [weak self] action, error in
            self?.didFinishLogin(action: action, error: error) ?? .nothing
        }

        preferredAccountNumberPublisher?
            .compactMap { $0 }
            .sink(receiveValue: { preferredAccountNumber in
                interactor.suggestPreferredAccountNumber?(preferredAccountNumber)
            })
            .store(in: &subscriptions)

        interactor.didCreateAccount = didCreateAccount

        navigationController.pushViewController(loginController, animated: animated)

        self.loginController = loginController

        setUpBreadcrumbs()
    }

    // MARK: - Private

    private func setUpBreadcrumbs() {
        loginController?.showInvalidAccessMethodView(breadcrumbsProvider.breadcrumbs.contains(.warning(.apiAccess)))

        let breadcrumbsObserver = BreadcrumbsBlockObserver(didUpdateBreadcrumbsHandler: { [weak self] in
            self?.loginController?.showInvalidAccessMethodView($0.contains(.warning(.apiAccess)))
        })
        self.breadcrumbsObserver = breadcrumbsObserver
        breadcrumbsProvider.add(observer: breadcrumbsObserver)
    }

    private func didFinishLogin(action: LoginAction, error: Error?) -> EndLoginAction {
        guard let error else {
            callDidFinishAfterDelay()
            return .nothing
        }

        // Warren enforces its concurrency cap server-side, so every login failure
        // (including a "too many devices" server response) surfaces as a normal
        // login error and returns focus to the account text field.
        if case .useExistingAccount = action {
            return .activateTextField
        }

        return .nothing
    }

    private func callDidFinishAfterDelay() {
        DispatchQueue.main.asyncAfter(deadline: .now() + .seconds(1)) { [weak self] in
            guard let self else { return }
            didFinish?(self)
        }
    }
}
