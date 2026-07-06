//
//  SceneDelegate.swift
//  MullvadVPN
//
//  Created by pronebird on 20/05/2022.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import WarrenLogging
import WarrenREST
import WarrenRustRuntime
import WarrenSettings
import WarrenTypes
import Operations
import UIKit

class SceneDelegate: UIResponder, UIWindowSceneDelegate, @preconcurrency SettingsMigrationUIHandler {
    private let logger = Logger(label: "SceneDelegate")

    var window: UIWindow?
    private var privacyOverlayWindow: UIWindow?
    private var isSceneConfigured = false

    private var appCoordinator: ApplicationCoordinator?
    private var accountDataThrottling: AccountDataThrottling?
    private var deviceDataThrottling: DeviceDataThrottling?

    private var tunnelObserver: TunnelObserver?

    private var appDelegate: AppDelegate {
        UIApplication.shared.delegate as! AppDelegate
    }

    private var accessMethodRepository: AccessMethodRepositoryProtocol {
        appDelegate.accessMethodRepository
    }

    private var tunnelManager: TunnelManager {
        appDelegate.tunnelManager
    }

    // MARK: - Private

    private func addTunnelObserver() {
        let tunnelObserver = TunnelBlockObserver(
            didLoadConfiguration: { [weak self] _ in
                self?.configureScene()
            },
            didUpdateDeviceState: { [weak self] _, deviceState, _ in
                self?.deviceStateDidChange(deviceState)
            }
        )

        self.tunnelObserver = tunnelObserver

        tunnelManager.addObserver(tunnelObserver)
    }

    private func configureScene() {
        guard !isSceneConfigured else { return }

        isSceneConfigured = true
        disableAnimationsIfNeeded()

        accountDataThrottling = AccountDataThrottling(tunnelManager: tunnelManager)
        deviceDataThrottling = DeviceDataThrottling(tunnelManager: tunnelManager)
        refreshLoginMetadata(forceUpdate: true)

        appCoordinator = ApplicationCoordinator(
            tunnelManager: tunnelManager,
            storePaymentManager: appDelegate.storePaymentManager,
            relayCacheTracker: appDelegate.relayCacheTracker,
            apiProxy: appDelegate.apiProxy,
            outgoingConnectionService: OutgoingConnectionService(
                outgoingConnectionProxy: OutgoingConnectionProxy(
                    urlSession: REST.makeURLSession(),
                    hostname: ApplicationConfiguration.hostName
                )
            ),
            appPreferences: appDelegate.appPreferences,
            accessMethodRepository: accessMethodRepository,
            ipOverrideRepository: appDelegate.ipOverrideRepository,
            relaySelectorWrapper: appDelegate.relaySelector,
            breadcrumbsProvider: appDelegate.breadcrumbsProvider
        )

        appCoordinator?.onShowSettings = { [weak self] in
            // Refresh account data and device each time user opens settings
            self?.refreshLoginMetadata(forceUpdate: true)
        }

        appCoordinator?.onNewNotificationSettings = { [weak self] notificationSettings in
            guard let self = self else { return }
            appDelegate.notificationSettingsListener.onNewSettings?(notificationSettings)
            NotificationManager.shared.updateNotifications()
        }

        appCoordinator?.onShowAccount = { [weak self] in
            // Refresh account data and device each time user opens account controller
            self?.refreshLoginMetadata(forceUpdate: true)
        }

        window?.rootViewController = appCoordinator?.rootViewController
        appCoordinator?.start()
    }

    private func disableAnimationsIfNeeded() {
        guard appDelegate.launchArguments.areAnimationsDisabled else { return }
        [privacyOverlayWindow, window]
            .compactMap { $0 }
            .forEach { $0.layer.speed = 100 }
    }

    private func setShowsPrivacyOverlay(_ showOverlay: Bool) {
        if showOverlay {
            privacyOverlayWindow?.isHidden = false
            privacyOverlayWindow?.makeKeyAndVisible()
        } else {
            privacyOverlayWindow?.isHidden = true
            window?.makeKeyAndVisible()
        }
    }

    private func deviceStateDidChange(_ deviceState: DeviceState) {
        switch deviceState {
        case .loggedOut, .revoked:
            resetLoginMetadataThrottling()

        case .loggedIn:
            break
        }
    }

    /**
     Refresh login metadata (account and device data) potentially throttling refresh requests based on recency of
     the last issued request.
    
     Account data is always refreshed when either settings or account are presented on screen, otherwise only when close
     to or past expiry.
    
     Both account and device data are refreshed regardless of other conditions when `forceUpdate` is `true`.
    
     For more information on exact timings used for throttling refresh requests refer to `AccountDataThrottling` and
     `DeviceDataThrottling` types.
     */
    private func refreshLoginMetadata(forceUpdate: Bool) {
        let condition: AccountDataThrottling.Condition

        if forceUpdate {
            condition = .always
        } else {
            let isPresentingSettings = appCoordinator?.isPresentingSettings ?? false
            let isPresentingAccount = appCoordinator?.isPresentingAccount ?? false

            condition = isPresentingSettings || isPresentingAccount ? .always : .whenCloseToExpiryAndBeyond
        }

        accountDataThrottling?.requestUpdate(condition: condition)
        deviceDataThrottling?.requestUpdate(forceUpdate: forceUpdate)
    }

    /**
     Reset throttling for login metadata making a subsequent refresh request execute unthrottled.
     */
    private func resetLoginMetadataThrottling() {
        accountDataThrottling?.reset()
        deviceDataThrottling?.reset()
    }

    // MARK: - UIWindowSceneDelegate

    func scene(
        _ scene: UIScene,
        willConnectTo session: UISceneSession,
        options connectionOptions: UIScene.ConnectionOptions
    ) {
        guard let windowScene = scene as? UIWindowScene else { return }
        let launchViewController = LaunchViewController(
            launchArguments: appDelegate.launchArguments,
            tunnelManager: tunnelManager,
            accessMethodRepository: accessMethodRepository)

        launchViewController.onAppReady = { [weak self] in
            guard let self = self else { return }
            if tunnelManager.isConfigurationLoaded {
                isSceneConfigured = false
                configureScene()
            }
        }

        window = UIWindow(windowScene: windowScene)
        window?.rootViewController = launchViewController

        privacyOverlayWindow = UIWindow(windowScene: windowScene)
        privacyOverlayWindow?.rootViewController = launchViewController
        privacyOverlayWindow?.windowLevel = .alert + 1

        window?.makeKeyAndVisible()
        addTunnelObserver()

        // A `warren://forum-login` deep link that cold-starts the app arrives
        // here rather than via `openURLContexts` (doc 55).
        if let url = connectionOptions.urlContexts.first?.url {
            handleForumLoginURL(url)
        }
    }

    func sceneDidDisconnect(_ scene: UIScene) {}

    func sceneDidBecomeActive(_ scene: UIScene) {
        if isSceneConfigured {
            refreshLoginMetadata(forceUpdate: false)
        }

        setShowsPrivacyOverlay(false)
    }

    func sceneWillResignActive(_ scene: UIScene) {
        setShowsPrivacyOverlay(true)
    }

    func sceneWillEnterForeground(_ scene: UIScene) {}

    func sceneDidEnterBackground(_ scene: UIScene) {}

    // MARK: - Community-forum wallet login deep link (doc 55)

    /// The single connect host accepted from a forum-login deep link (Rust
    /// re-validates; this is a fail-fast, not the security boundary).
    private static let forumLoginAllowedHost = "connect.warrenbrowse.com"

    func scene(_ scene: UIScene, openURLContexts URLContexts: Set<UIOpenURLContext>) {
        guard let url = URLContexts.first?.url else { return }
        handleForumLoginURL(url)
    }

    /// Parses `warren://forum-login?sid=..&host=..` and, when valid, shows the
    /// consent prompt (the app NEVER signs into the forum silently). Non-forum
    /// URLs are ignored.
    private func handleForumLoginURL(_ url: URL) {
        guard let link = Self.parseForumLogin(url) else { return }
        DispatchQueue.main.async { [weak self] in
            self?.presentForumLoginConsent(sid: link.sid, host: link.host)
        }
    }

    private static func parseForumLogin(_ url: URL) -> (sid: String, host: String)? {
        guard url.scheme == "warren", url.host == "forum-login",
              let components = URLComponents(url: url, resolvingAgainstBaseURL: false),
              let items = components.queryItems else { return nil }
        let params = Dictionary(
            items.compactMap { item in item.value.map { (item.name, $0) } },
            uniquingKeysWith: { first, _ in first })
        guard let sid = params["sid"], let host = params["host"],
              host == forumLoginAllowedHost,
              sid.count == 32,
              sid.allSatisfy({ $0.isHexDigit && ($0.isNumber || $0.isLowercase) })
        else { return nil }
        return (sid, host)
    }

    private func presentForumLoginConsent(sid: String, host: String) {
        guard let presenter = topViewController() else { return }
        let alert = UIAlertController(
            title: NSLocalizedString(
                "Sign in to the Warren community forum?",
                comment: "Forum login consent prompt title"),
            message: NSLocalizedString(
                """
                Your app will sign a one time challenge with your wallet key to \
                prove it is you. No email and no password are used, and you appear \
                under an anonymous handle that cannot be linked to your Warren \
                account. Only approve if you started this sign in.
                """,
                comment: "Forum login consent prompt body"),
            preferredStyle: .alert)
        alert.addAction(UIAlertAction(
            title: NSLocalizedString("Cancel", comment: ""), style: .cancel))
        alert.addAction(UIAlertAction(
            title: NSLocalizedString("Approve", comment: ""),
            style: .default,
            handler: { [weak self] _ in self?.performForumLogin(sid: sid, host: host) }))
        presenter.present(alert, animated: true)
    }

    /// Loads the wallet seed silently and signs + POSTs the challenge in Rust
    /// off the main thread (the Keychain item is `WhenUnlockedThisDeviceOnly`,
    /// no biometric gate, matching the tunnel which signs with the same key).
    private func performForumLogin(sid: String, host: String) {
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            let outcome: WarrenForumLoginOutcome
            if let mnemonic = try? WarrenWalletKeychain.load(),
               let wallet = try? WarrenWallet.fromMnemonic(mnemonic) {
                defer { wallet.forgetSecret() }
                outcome = WarrenAccountClient.forumLogin(seed: wallet.seed, sid: sid, host: host)
            } else {
                outcome = .failed
            }
            DispatchQueue.main.async { self?.presentForumLoginResult(outcome) }
        }
    }

    private func presentForumLoginResult(_ outcome: WarrenForumLoginOutcome) {
        guard let presenter = topViewController() else { return }
        let message: String
        switch outcome {
        case .approved:
            message = NSLocalizedString(
                "Signed in to the Warren forum.", comment: "Forum login success")
        case .subscriptionRequired:
            message = NSLocalizedString(
                "Forum access requires a Warren subscription. This wallet has never subscribed.",
                comment: "Forum login refused, no subscription")
        case .failed:
            message = NSLocalizedString(
                "Sign in failed. Please try again in a moment.", comment: "Forum login failed")
        }
        let alert = UIAlertController(title: nil, message: message, preferredStyle: .alert)
        alert.addAction(UIAlertAction(title: NSLocalizedString("OK", comment: ""), style: .default))
        presenter.present(alert, animated: true)
    }

    private func topViewController() -> UIViewController? {
        var top = window?.rootViewController
        while let presented = top?.presentedViewController { top = presented }
        return top
    }

    // MARK: - SettingsMigrationUIHandler

    func showMigrationError(_ error: Error, completionHandler: @escaping () -> Void) {
        guard let appCoordinator else {
            completionHandler()
            return
        }

        let presentation = AlertPresentation(
            id: "settings-migration-error-alert",
            title: NSLocalizedString("Settings migration error", comment: ""),
            message: Self.migrationErrorReason(error),
            buttons: [
                AlertAction(
                    title: NSLocalizedString("Got it!", comment: ""),
                    style: .default,
                    handler: {
                        completionHandler()
                    }
                )
            ]
        )

        let presenter = AlertPresenter(context: appCoordinator)
        presenter.showAlert(presentation: presentation, animated: true)
    }

    private static func migrationErrorReason(_ error: Error) -> String {
        if error is UnsupportedSettingsVersionError {
            return NSLocalizedString(
                """
                The version of settings stored on device is unrecognized.\
                Settings will be reset to defaults and the device will be logged out.
                """,
                comment: ""
            )
        } else {
            return NSLocalizedString(
                """
                Internal error occurred. Settings will be reset to defaults and device logged out.
                """,
                comment: ""
            )
        }
    }
}
