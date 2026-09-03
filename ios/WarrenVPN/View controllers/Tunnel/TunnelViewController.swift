//
//  TunnelViewController.swift
//  MullvadVPN
//
//  Created by Jon Petersson on 2024-12-10.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import Combine
import MapKit
import WarrenLogging
import WarrenREST
import WarrenRustRuntime
import WarrenSettings
import WarrenTypes
import SwiftUI

/// Account line resting on the scenery's bottom scrim (desktop AppMainFooter):
/// the copyable shortened wallet pubkey on the left, the remaining time on the
/// right. No chrome of its own, the backdrop scrim grounds it.
private final class WarrenMainFooterViewModel: ObservableObject {
    @Published var pubkey: String?
    @Published var expiry: Date?
}

private struct WarrenMainFooterView: View {
    @ObservedObject var viewModel: WarrenMainFooterViewModel
    @State private var justCopied = false

    var body: some View {
        HStack(alignment: .center) {
            if let pubkey = viewModel.pubkey {
                Button {
                    UIPasteboard.general.string = pubkey
                    withAnimation { justCopied = true }
                    Task {
                        try? await Task.sleep(nanoseconds: 2_000_000_000)
                        withAnimation { justCopied = false }
                    }
                } label: {
                    HStack(spacing: 4) {
                        Text(Self.shorten(pubkey))
                            .font(.system(.footnote, design: .monospaced))
                            .foregroundStyle(.white.opacity(justCopied ? 0.8 : 0.6))
                        Image(systemName: justCopied ? "checkmark" : "doc.on.doc")
                            .font(.caption2)
                            .foregroundStyle(
                                justCopied ? Color(uiColor: .successColor) : .white.opacity(0.4))
                    }
                }
                .accessibilityLabel(LocalizedStringKey("Copy public key"))
            }
            Spacer()
            if let expiry = viewModel.expiry {
                Text(Self.timeLeftText(expiry: expiry))
                    .font(.footnote)
                    .foregroundStyle(.white.opacity(0.6))
            }
        }
        .shadow(color: .black.opacity(0.55), radius: 3, x: 0, y: 1)
        .padding(.horizontal, 18)
        .padding(.bottom, 7)
    }

    // head...tail form (U+2026), the Polkadot short-address style used by the
    // desktop footer.
    private static func shorten(_ pubkey: String) -> String {
        guard pubkey.count > 13 else { return pubkey }
        return "\(pubkey.prefix(6))\u{2026}\(pubkey.suffix(6))"
    }

    private static func timeLeftText(expiry: Date) -> String {
        String(
            format: NSLocalizedString("Time left: %@", comment: ""),
            CustomDateComponentsFormatting.localizedString(
                from: Date(),
                to: expiry,
                unitsStyle: .full
            ) ?? ""
        )
    }
}

class TunnelViewController: UIViewController, RootContainment {
    private let logger = Logger(label: "TunnelViewController")
    private let interactor: TunnelViewControllerInteractor
    private var tunnelState: TunnelState = .disconnected
    private var connectionViewViewModel: ConnectionViewViewModel
    private var indicatorsViewViewModel: FeatureIndicatorsViewModel
    private var connectionView: ConnectionView
    private var connectionController: UIHostingController<ConnectionView>?
    private let footerViewModel = WarrenMainFooterViewModel()
    private var footerController: UIHostingController<WarrenMainFooterView>?

    // Warren multi-exit failover notification surface. Reads App Group
    // UserDefaults keys written by the tunnel extension on exit-down
    // recovery. Until the tunnel extension writes these keys, this
    // observer simply never fires.
    private lazy var appGroupEvents = WarrenAppGroupEvents(
        suiteName: ApplicationConfiguration.securityGroupIdentifier
    )
    private var failoverBannerController: UIHostingController<WarrenFailoverBannerView>?
    private var failoverCancellable: Combine.AnyCancellable?
    private var failoverHideTask: Task<Void, Never>?
    private var lastShownFailoverDate: Date?

    // Warren exit-pubkey TOFU mismatch surface. The tunnel extension fails
    // the connection closed when an exit serves a pubkey that differs from
    // the locally pinned one and broadcasts the mismatch through the App
    // Group; here we present the Trust / Report / Reject alert.
    private var pinMismatchCancellable: Combine.AnyCancellable?
    private var lastShownPinMismatchDate: Date?
    private var pinMismatchAlertController: AlertViewController?

    var shouldShowSelectLocationPicker: (() -> Void)?
    var shouldShowCancelTunnelAlert: (() -> Void)?
    var shouldShowSettingsForFeature: ((FeatureType) -> Void)?

    let activityIndicator: SpinnerActivityIndicatorView = {
        let activityIndicator = SpinnerActivityIndicatorView(style: .large)
        activityIndicator.translatesAutoresizingMaskIntoConstraints = false
        activityIndicator.tintColor = .white
        activityIndicator.setContentHuggingPriority(.defaultHigh, for: .horizontal)
        activityIndicator.setContentCompressionResistancePriority(.defaultHigh, for: .horizontal)
        return activityIndicator
    }()

    // Backdrop switch: the per-country scenery (desktop Bula connect screen
    // port) is the default; set to false to fall back to the legacy 3D map.
    // MapViewController is kept fully intact behind this flag on purpose.
    private static let usesSceneryBackdrop = true

    private let mapViewController = MapViewController()
    private let sceneryViewController = SceneryViewController()

    override var preferredStatusBarStyle: UIStatusBarStyle {
        // Dark status bar over the bright scenery sky, matching the black
        // header content (desktop main header dark tone).
        .darkContent
    }

    var preferredHeaderBarPresentation: HeaderBarPresentation {
        switch interactor.deviceState {
        case .loggedIn, .revoked:
            // The phase color moved into the scenery + connection card; the
            // header floats transparent over the artwork with dark content
            // (desktop MainView), instead of an opaque state-colored bar.
            return HeaderBarPresentation(
                style: .transparent,
                showsDivider: false,
                tone: .dark
            )
        case .loggedOut:
            return HeaderBarPresentation(style: .default, showsDivider: true)
        }
    }

    var prefersHeaderBarHidden: Bool {
        false
    }

    var prefersDeviceInfoBarHidden: Bool {
        // The account info lives in the bottom footer over the scenery scrim
        // (desktop AppMainFooter), not in a header sub-row.
        true
    }

    var prefersNotificationBarHidden: Bool {
        false
    }

    init(interactor: TunnelViewControllerInteractor) {
        self.interactor = interactor

        tunnelState = interactor.tunnelStatus.state
        connectionViewViewModel = ConnectionViewViewModel(
            tunnelStatus: interactor.tunnelStatus,
            relayConstraints: interactor.tunnelSettings.relayConstraints,
            relayCache: RelayCache(cacheDirectory: ApplicationConfiguration.containerURL),
            customListRepository: CustomListRepository()
        )
        connectionViewViewModel.isStandingDownForHigherEnvironment =
            interactor.isStandingDownForHigherEnvironment
        indicatorsViewViewModel = FeatureIndicatorsViewModel(
            tunnelSettings: interactor.tunnelSettings,
            tunnelStatus: interactor.tunnelStatus
        )

        connectionView = ConnectionView(
            connectionViewModel: connectionViewViewModel,
            indicatorsViewModel: indicatorsViewViewModel
        )

        super.init(nibName: nil, bundle: nil)
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override func viewDidLoad() {
        super.viewDidLoad()
        indicatorsViewViewModel.onFeaturePressed = { [weak self] feature in
            self?.shouldShowSettingsForFeature?(feature)
        }

        interactor.didUpdateDeviceState = { [weak self] _, _ in
            self?.setNeedsHeaderBarStyleAppearanceUpdate()
            self?.refreshFooter()
        }

        interactor.didUpdateTunnelStatus = { [weak self] tunnelStatus in
            self?.connectionViewViewModel.update(tunnelStatus: tunnelStatus)
            self?.setTunnelState(tunnelStatus.state, animated: true)
            self?.indicatorsViewViewModel.tunnelState = tunnelStatus.state
            self?.indicatorsViewViewModel.observedState = tunnelStatus.observedState
            self?.view.setNeedsLayout()
        }

        interactor.didUpdateTunnelSettings = { [weak self] tunnelSettings in
            guard let self else { return }
            indicatorsViewViewModel.tunnelSettings = tunnelSettings
            connectionViewViewModel.relayConstraints = tunnelSettings.relayConstraints
            // Standing down and re-enabling both write the kill-switch
            // setting, so this is the settling point where the buttons learn
            // the record moved.
            connectionViewViewModel.isStandingDownForHigherEnvironment =
                interactor.isStandingDownForHigherEnvironment
        }

        connectionView.action = { [weak self] action in
            switch action {
            case .connect:
                self?.logger.debug("User tapped connect button")
                self?.interactor.startTunnel()

            case .cancel:
                self?.logger.debug("User tapped cancel button")
                if case .waitingForConnectivity(.noConnection) = self?.interactor.tunnelStatus.state {
                    self?.shouldShowCancelTunnelAlert?()
                } else {
                    self?.interactor.stopTunnel()
                }

            case .disconnect:
                self?.logger.debug("User tapped disconnect button")
                self?.interactor.stopTunnel()

            case .reconnect:
                self?.logger.debug("User tapped reconnect button")
                self?.interactor.reconnectTunnel(selectNewRelay: true)

            case .selectLocation:
                self?.shouldShowSelectLocationPicker?()

            case .shuffleLocation:
                self?.logger.debug("User tapped shuffle location button")
                self?.interactor.shuffleExitLocation()
            }
        }

        if Self.usesSceneryBackdrop {
            addSceneryController()
        } else {
            addMapController()
        }
        addActivityIndicator()
        addFooterView()
        addConnectionView()
        refreshFooter()
        updateBackdrop(animated: false)
        subscribeToFailoverEvents()
        subscribeToPinMismatchEvents()
    }

    func setMainContentHidden(_ isHidden: Bool, animated: Bool) {
        let actions = {
            _ = self.connectionView.opacity(isHidden ? 0 : 1)
        }

        if animated {
            UIView.animate(withDuration: 0.25, animations: actions)
        } else {
            actions()
        }
    }

    // MARK: - Private

    private func setTunnelState(_ tunnelState: TunnelState, animated: Bool) {
        self.tunnelState = tunnelState

        setNeedsHeaderBarStyleAppearanceUpdate()

        guard isViewLoaded else { return }

        updateBackdrop(animated: animated)
    }

    private func updateBackdrop(animated: Bool) {
        if Self.usesSceneryBackdrop {
            updateScenery(animated: animated)
        } else {
            updateMap(animated: animated)
        }
    }

    private func updateScenery(animated: Bool) {
        switch tunnelState {
        case .connecting, .reconnecting, .negotiatingEphemeralPeer, .pendingReconnect:
            activityIndicator.startAnimating()
        default:
            activityIndicator.stopAnimating()
        }

        sceneryViewController.update(
            phase: ConnectionPhase(tunnelState: tunnelState),
            exitCountry: tunnelState.relays?.exit.location.country,
            animated: animated
        )
    }

    private func updateMap(animated: Bool) {
        switch tunnelState {
        case let .connecting(tunnelRelays, _, _):
            mapViewController.removeLocationMarker()
            mapViewController.setCenter(tunnelRelays?.exit.location.geoCoordinate, animated: animated)
            activityIndicator.startAnimating()

        case let .reconnecting(tunnelRelays, _, _), let .negotiatingEphemeralPeer(tunnelRelays, _, _, _):
            activityIndicator.startAnimating()
            mapViewController.removeLocationMarker()
            mapViewController.setCenter(tunnelRelays.exit.location.geoCoordinate, animated: animated)

        case let .connected(tunnelRelays, _, _):
            let center = tunnelRelays.exit.location.geoCoordinate
            mapViewController.setCenter(center, animated: animated)
            activityIndicator.stopAnimating()
            mapViewController.addLocationMarker(coordinate: center)

        case .pendingReconnect:
            activityIndicator.startAnimating()
            mapViewController.removeLocationMarker()

        case .waitingForConnectivity, .error:
            activityIndicator.stopAnimating()
            mapViewController.removeLocationMarker()

        case .disconnected, .disconnecting:
            activityIndicator.stopAnimating()
            mapViewController.removeLocationMarker()
            mapViewController.setCenter(nil, animated: animated)
        }
    }

    private func addMapController() {
        let mapView = mapViewController.view!

        addChild(mapViewController)
        mapViewController.alignmentView = activityIndicator
        mapViewController.didMove(toParent: self)

        view.addConstrainedSubviews([mapView]) {
            mapView.pinEdgesToSuperview()
        }
    }

    private func addSceneryController() {
        let sceneryView = sceneryViewController.view!

        addChild(sceneryViewController)
        sceneryViewController.didMove(toParent: self)

        view.addConstrainedSubviews([sceneryView]) {
            sceneryView.pinEdgesToSuperview()
        }
    }

    /// Computes a constraint multiplier based on the screen size
    private func computeHeightBreakpointMultiplier() -> CGFloat {
        let screenBounds = UIWindow().screen.coordinateSpace.bounds
        return screenBounds.height < 700 ? 2.0 : 1.5
    }

    private func addActivityIndicator() {
        // If the device doesn't have a lot of vertical screen estate, center the progress view higher on the map
        // so the connection view details do not shadow it unless fully expanded if possible
        let heightConstraintMultiplier = computeHeightBreakpointMultiplier()

        let verticalCenteredAnchor = activityIndicator.centerYAnchor.anchorWithOffset(to: view.centerYAnchor)
        view.addConstrainedSubviews([activityIndicator]) {
            activityIndicator.centerXAnchor.constraint(equalTo: view.centerXAnchor)
            verticalCenteredAnchor.constraint(
                equalTo: activityIndicator.heightAnchor,
                multiplier: heightConstraintMultiplier
            )
        }
    }

    private func addFooterView() {
        let footerController = UIHostingController(
            rootView: WarrenMainFooterView(viewModel: footerViewModel))
        self.footerController = footerController

        let footerProxy = footerController.view!
        footerProxy.backgroundColor = .clear

        addChild(footerController)
        footerController.didMove(toParent: self)
        view.addConstrainedSubviews([footerProxy]) {
            footerProxy.leadingAnchor.constraint(equalTo: view.leadingAnchor)
            footerProxy.trailingAnchor.constraint(equalTo: view.trailingAnchor)
            footerProxy.bottomAnchor.constraint(equalTo: view.safeAreaLayoutGuide.bottomAnchor)
        }
    }

    private func refreshFooter() {
        let accountData = interactor.deviceState.accountData
        footerViewModel.pubkey = accountData?.number
        footerViewModel.expiry = accountData?.isExpired == false ? accountData?.expiry : nil
    }

    private func addConnectionView() {
        let connectionController = UIHostingController(rootView: connectionView)
        self.connectionController = connectionController

        let connectionViewProxy = connectionController.view!
        connectionViewProxy.backgroundColor = .clear

        addChild(connectionController)
        connectionController.didMove(toParent: self)
        view.addConstrainedSubviews([activityIndicator, connectionViewProxy]) {
            connectionViewProxy.pinEdgesToSuperview(.init([.top(0), .leading(0), .trailing(0)]))
            connectionViewProxy.bottomAnchor.constraint(
                equalTo: footerController?.view.topAnchor ?? view.bottomAnchor)
        }
    }

    // MARK: - Warren failover banner

    private func subscribeToFailoverEvents() {
        failoverCancellable = appGroupEvents.$lastFailover
            .receive(on: DispatchQueue.main)
            .sink { [weak self] event in
                guard let self, let event, event.isFresh else { return }
                // Skip if we already surfaced this exact event in this
                // VC's lifetime (UserDefaults observers can re-fire on
                // unrelated key changes).
                if let last = self.lastShownFailoverDate, last == event.occurredAt {
                    return
                }
                self.lastShownFailoverDate = event.occurredAt
                self.showFailoverBanner(event)
            }
    }

    private func showFailoverBanner(_ event: WarrenFailoverEvent) {
        // Cancel any pending auto-hide so the new banner is not pre-empted
        // by a stale dismiss task from a previous failover.
        failoverHideTask?.cancel()
        hideFailoverBanner(animated: false)

        let banner = WarrenFailoverBannerView(
            info: WarrenFailoverBannerInfo(country: event.country, occurredAt: event.occurredAt),
            onDismiss: { [weak self] in self?.hideFailoverBanner(animated: true) }
        )

        let host = UIHostingController(rootView: banner)
        host.view.backgroundColor = .clear
        host.view.alpha = 0
        host.view.translatesAutoresizingMaskIntoConstraints = false
        addChild(host)
        view.addSubview(host.view)
        host.didMove(toParent: self)
        NSLayoutConstraint.activate([
            host.view.leadingAnchor.constraint(equalTo: view.layoutMarginsGuide.leadingAnchor, constant: 8),
            host.view.trailingAnchor.constraint(equalTo: view.layoutMarginsGuide.trailingAnchor, constant: -8),
            host.view.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor, constant: 8),
        ])
        failoverBannerController = host

        UIView.animate(withDuration: 0.25) {
            host.view.alpha = 1
        }

        failoverHideTask = Task { [weak self] in
            try? await Task.sleep(nanoseconds: 10_000_000_000)
            await MainActor.run {
                guard let self else { return }
                self.hideFailoverBanner(animated: true)
            }
        }
    }

    private func hideFailoverBanner(animated: Bool) {
        guard let host = failoverBannerController else { return }
        let cleanup = {
            host.willMove(toParent: nil)
            host.view.removeFromSuperview()
            host.removeFromParent()
        }
        if animated {
            UIView.animate(
                withDuration: 0.2,
                animations: { host.view.alpha = 0 },
                completion: { _ in cleanup() }
            )
        } else {
            cleanup()
        }
        failoverBannerController = nil
    }

    // MARK: - Warren exit-pubkey TOFU mismatch

    private func subscribeToPinMismatchEvents() {
        pinMismatchCancellable = appGroupEvents.$lastPinMismatch
            .receive(on: DispatchQueue.main)
            .sink { [weak self] event in
                guard let self, let event, event.isFresh else { return }
                // Skip an event we already surfaced this VC lifetime (the
                // UserDefaults observer can re-fire on unrelated changes).
                if let last = self.lastShownPinMismatchDate, last == event.occurredAt {
                    return
                }
                self.lastShownPinMismatchDate = event.occurredAt
                self.showPinMismatchAlert(event.mismatch)
            }
    }

    /// Path to the App Group exit-pubkey TOFU pin table written by the
    /// tunnel extension (sibling of the multi-hop generation file).
    private var pinStorePath: String {
        ApplicationConfiguration.containerURL.appendingPathComponent("warren-exit-pins.json").path
    }

    private func showPinMismatchAlert(_ mismatch: WarrenPinMismatch) {
        // Avoid stacking duplicate alerts if one is already up.
        guard pinMismatchAlertController == nil else { return }

        let title = NSLocalizedString(
            "Server identity changed",
            tableName: "Settings",
            comment: "Title of the alert shown when a Warren exit serves a different exit pubkey than the one previously pinned."
        )
        let messageLines = [
            NSLocalizedString(
                "The Warren exit server you previously trusted now presents a different cryptographic identity.",
                tableName: "Settings",
                comment: "First line of the exit-pubkey mismatch alert body."
            ),
            NSLocalizedString(
                "This usually means the operator rotated the key, but it can also indicate that the server has been replaced or compromised. Refuse if you did not expect a change.",
                tableName: "Settings",
                comment: "Second line of the exit-pubkey mismatch alert body."
            ),
        ]

        let presentation = AlertPresentation(
            id: "warren-pubkey-mismatch-alert",
            icon: .warning,
            title: title,
            message: messageLines.joined(separator: "\n\n"),
            buttons: [
                AlertAction(
                    title: NSLocalizedString(
                        "Trust new key",
                        tableName: "Settings",
                        comment: "Button that pins the newly observed exit pubkey and reconnects."
                    ),
                    style: .default,
                    handler: { [weak self] in
                        self?.handlePinMismatchTrust(mismatch)
                    }
                ),
                AlertAction(
                    title: NSLocalizedString(
                        "Report to Warren",
                        tableName: "Settings",
                        comment: "Button that opens the support page to report a suspicious exit-pubkey change."
                    ),
                    style: .default,
                    handler: { [weak self] in
                        self?.handlePinMismatchReport()
                    }
                ),
                AlertAction(
                    title: NSLocalizedString(
                        "Reject (disconnect)",
                        tableName: "Settings",
                        comment: "Button that dismisses the mismatch alert and stays disconnected."
                    ),
                    style: .destructive,
                    handler: { [weak self] in
                        self?.handlePinMismatchReject()
                    }
                ),
            ]
        )

        let alert = AlertViewController(presentation: presentation)
        alert.onDismiss = { [weak self] in
            self?.pinMismatchAlertController = nil
        }
        pinMismatchAlertController = alert
        present(alert, animated: true)
    }

    private func dismissPinMismatchAlert() {
        appGroupEvents.clearPinMismatch()
        pinMismatchAlertController?.dismiss(animated: true)
        pinMismatchAlertController = nil
    }

    private func handlePinMismatchTrust(_ mismatch: WarrenPinMismatch) {
        let trusted = WarrenQuinnAdapter.pinTrust(
            pinStorePath: pinStorePath,
            exitIdHex: mismatch.exitId,
            pubkeyHex: mismatch.observed,
            country: mismatch.country
        )
        if !trusted {
            logger.error("Failed to trust new exit pubkey for exit \(mismatch.exitId)")
        }
        dismissPinMismatchAlert()
        // The tunnel failed closed; reconnect now that the new key is
        // pinned. Keep the same relay selection (no new relay).
        interactor.reconnectTunnel(selectNewRelay: false)
    }

    private func handlePinMismatchReport() {
        let language = Bundle.preferredLocalizations(from: ["en"]).first ?? "en"
        UIApplication.shared.open(ApplicationConfiguration.faqAndGuidesURL(for: language))
        // Reporting does not trust the key; stay disconnected.
        dismissPinMismatchAlert()
    }

    private func handlePinMismatchReject() {
        // The tunnel already failed closed; just clear the pending mismatch.
        dismissPinMismatchAlert()
    }
}
