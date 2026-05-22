//
//  WarrenWalletCoordinator.swift
//  WarrenVPN
//
//  Created by Warren on 2026-05-21.
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  Coordinator for the Warren wallet onboarding + restore + backup
//  flows. Follows the Mullvad iOS Coordinator pattern (see
//  `Routing/Coordinator.swift`) so it composes cleanly with
//  `ApplicationCoordinator` / `LoginCoordinator` / `WelcomeCoordinator`.
//

import Routing
import UIKit
import WarrenLogging

/// Entry points the coordinator can navigate to.
public enum WarrenWalletEntryPoint {
    /// Show the generate-then-confirm flow first ; the user may
    /// branch to import via the secondary action.
    case generate
    /// Show the 12-word input grid directly (e.g. from a "Restore"
    /// CTA on the login screen).
    case importExisting
    /// Show the backup phrase (Face ID gated) ; e.g. from Settings.
    case backup
}

final class WarrenWalletCoordinator: Coordinator, Presentable {
    private let logger = Logger(label: "WarrenWalletCoordinator")
    private let navigationController: UINavigationController
    private let interactor: WarrenWalletInteractor
    private let entryPoint: WarrenWalletEntryPoint

    /// Called when the wallet flow completes successfully (wallet
    /// persisted in Keychain) or is cancelled by the user.
    var didFinish: (@MainActor @Sendable (WarrenWalletCoordinator, Bool) -> Void)?

    var presentedViewController: UIViewController {
        navigationController
    }

    init(
        navigationController: UINavigationController,
        interactor: WarrenWalletInteractor = WarrenWalletInteractor(),
        entryPoint: WarrenWalletEntryPoint
    ) {
        self.navigationController = navigationController
        self.interactor = interactor
        self.entryPoint = entryPoint
    }

    func start(animated: Bool) {
        switch entryPoint {
        case .generate:
            showGenerate(animated: animated)
        case .importExisting:
            showImport(animated: animated)
        case .backup:
            showBackup(animated: animated)
        }
    }

    // MARK: - Routes

    private func showGenerate(animated: Bool) {
        let controller = WarrenWalletGenerateViewController(interactor: interactor)
        controller.delegate = self
        navigationController.pushViewController(controller, animated: animated)
    }

    private func showImport(animated: Bool) {
        let controller = WarrenWalletImportViewController(interactor: interactor)
        controller.delegate = self
        navigationController.pushViewController(controller, animated: animated)
    }

    private func showBackup(animated: Bool) {
        let controller = WarrenWalletBackupViewController(interactor: interactor)
        controller.delegate = self
        navigationController.pushViewController(controller, animated: animated)
    }

    // MARK: - Helpers

    private func finish(success: Bool) {
        didFinish?(self, success)
    }
}

extension WarrenWalletCoordinator: @preconcurrency WarrenWalletGenerateViewControllerDelegate {
    func walletGenerateController(
        _ controller: WarrenWalletGenerateViewController,
        didConfirmMnemonic mnemonic: String
    ) {
        interactor.saveMnemonic(mnemonic) { [weak self] result in
            guard let self else { return }
            switch result {
            case .success:
                self.logger.info("Wallet generated + persisted to Keychain")
                self.finish(success: true)
            case .failure(let error):
                self.logger.error("Failed to persist generated wallet: \(error)")
                self.finish(success: false)
            }
        }
    }

    func walletGenerateControllerDidCancel(_ controller: WarrenWalletGenerateViewController) {
        finish(success: false)
    }
}

extension WarrenWalletCoordinator: @preconcurrency WarrenWalletImportViewControllerDelegate {
    func walletImportController(
        _ controller: WarrenWalletImportViewController,
        didImportMnemonic mnemonic: String
    ) {
        logger.info("Wallet imported + persisted to Keychain")
        finish(success: true)
    }

    func walletImportControllerDidCancel(_ controller: WarrenWalletImportViewController) {
        finish(success: false)
    }
}

extension WarrenWalletCoordinator: @preconcurrency WarrenWalletBackupViewControllerDelegate {
    func walletBackupControllerDidFinish(_ controller: WarrenWalletBackupViewController) {
        finish(success: true)
    }
}
