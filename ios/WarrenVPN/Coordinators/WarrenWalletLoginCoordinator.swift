//
//  WarrenWalletLoginCoordinator.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  Login coordinator for the wallet identity model: a Create-or-Restore
//  chooser that drives the existing `WarrenWalletCoordinator` generate /
//  import flows. Reached when onboarding is complete but no wallet is
//  present (fresh install handled by the onboarding wizard; this is the
//  post-logout re-entry, matching the desktop login screen).
//

import Routing
import SwiftUI
import UIKit
import WarrenLogging

final class WarrenWalletLoginCoordinator: Coordinator, Presentable {
    private let logger = Logger(label: "WarrenWalletLoginCoordinator")
    let navigationController: UINavigationController
    private let interactor: WarrenWalletInteractor

    /// Fired once a wallet exists in the Keychain (created or restored).
    var didFinish: (@MainActor @Sendable (WarrenWalletLoginCoordinator) -> Void)?

    var presentedViewController: UIViewController {
        navigationController
    }

    init(
        navigationController: UINavigationController = UINavigationController(),
        interactor: WarrenWalletInteractor = WarrenWalletInteractor()
    ) {
        self.navigationController = navigationController
        self.interactor = interactor
    }

    func start(animated: Bool) {
        let view = WarrenWalletLoginView(
            onCreate: { [weak self] in self?.presentWalletGenerate() },
            onRestore: { [weak self] in self?.presentWalletImport() }
        )
        let host = UIHostingController(rootView: view)
        host.view.backgroundColor = .Warren.navy
        host.modalPresentationStyle = .fullScreen
        navigationController.setViewControllers([host], animated: false)
        navigationController.setNavigationBarHidden(true, animated: false)
    }

    // MARK: - Routes

    private func presentWalletGenerate() {
        present(entryPoint: .generate)
    }

    private func presentWalletImport() {
        present(entryPoint: .importExisting)
    }

    private func present(entryPoint: WarrenWalletEntryPoint) {
        let coordinator = WarrenWalletCoordinator(
            navigationController: navigationController,
            interactor: interactor,
            entryPoint: entryPoint
        )
        coordinator.didFinish = { [weak self] coord, success in
            guard let self else { return }
            coord.removeFromParent()
            if success {
                self.finish()
            } else {
                // User backed out of the wallet flow; restore the chooser.
                self.navigationController.popToRootViewController(animated: true)
                self.navigationController.setNavigationBarHidden(true, animated: false)
            }
        }
        addChild(coordinator)
        navigationController.setNavigationBarHidden(false, animated: false)
        coordinator.start(animated: true)
    }

    private func finish() {
        didFinish?(self)
    }
}

/// Create-or-Restore chooser shown on the wallet login screen, mirroring
/// the desktop `LoginView` pick step: the Warren lockup above
/// "Welcome to Warren", then the create / restore choice.
private struct WarrenWalletLoginView: View {
    var onCreate: () -> Void
    var onRestore: () -> Void

    var body: some View {
        VStack(spacing: 0) {
            Spacer()

            Image("WarrenWordmark")
                .renderingMode(.template)
                .resizable()
                .scaledToFit()
                .frame(height: 40)
                .foregroundColor(.white)
                .accessibilityHidden(true)

            VStack(alignment: .leading, spacing: 12) {
                Text(String(localized: "Welcome to Warren", table: "Onboarding"))
                    .font(.warrenBig)
                    .foregroundColor(.white)

                Text(String(localized: "Warren uses a non-custodial wallet: your account is a 12-word recovery phrase that only you hold.", table: "Onboarding"))
                    .font(.warrenSmall)
                    .foregroundColor(.white.opacity(0.7))
                    .fixedSize(horizontal: false, vertical: true)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal, 24)
            .padding(.top, 32)

            Spacer()

            VStack(spacing: 12) {
                Button(action: onCreate) {
                    Text(String(localized: "Create a new account", table: "Onboarding"))
                        .font(.warrenSmallSemiBold)
                        .frame(maxWidth: .infinity)
                        .padding(.vertical, 16)
                        .background(Color.Warren.success)
                        .foregroundColor(.white)
                        .cornerRadius(12)
                }
                .accessibilityAddTraits(.isButton)
                .accessibilityIdentifier(AccessibilityIdentifier.walletCreateButton.asString)

                Button(action: onRestore) {
                    Text(String(localized: "I already have an account", table: "Onboarding"))
                        .font(.warrenSmall)
                        .frame(maxWidth: .infinity)
                        .padding(.vertical, 14)
                        .foregroundColor(.white.opacity(0.85))
                        .overlay(
                            RoundedRectangle(cornerRadius: 12)
                                .stroke(Color.white.opacity(0.25), lineWidth: 1)
                        )
                }
                .accessibilityAddTraits(.isButton)
                .accessibilityIdentifier(AccessibilityIdentifier.walletRestoreButton.asString)
            }
            .padding(.horizontal, 24)
            .padding(.bottom, 32)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(Color.Warren.navy.ignoresSafeArea())
        // NOTE: do NOT put an accessibilityIdentifier on this container.
        // In SwiftUI a container identifier propagates to every descendant
        // and clobbers the per-button ids below (walletCreateButton /
        // walletRestoreButton), which UI tests query directly. The chooser
        // is detected by the presence of the create button instead.
    }
}
