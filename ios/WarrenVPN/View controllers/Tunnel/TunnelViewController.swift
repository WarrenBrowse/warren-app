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
import WarrenSettings
import WarrenTypes
import SwiftUI

class TunnelViewController: UIViewController, RootContainment {
    private let logger = Logger(label: "TunnelViewController")
    private let interactor: TunnelViewControllerInteractor
    private var tunnelState: TunnelState = .disconnected
    private var connectionViewViewModel: ConnectionViewViewModel
    private var indicatorsViewViewModel: FeatureIndicatorsViewModel
    private var connectionView: ConnectionView
    private var connectionController: UIHostingController<ConnectionView>?

    // Warren multi-exit failover notification surface. Reads App Group
    // UserDefaults keys written by the tunnel extension on exit-down
    // recovery (cf. `.planning/c4-packet-tunnel-provider-quinn-design.md`
    // §2.3). Until C.4 lands, this observer simply never fires.
    private lazy var appGroupEvents = WarrenAppGroupEvents(
        suiteName: ApplicationConfiguration.securityGroupIdentifier
    )
    private var failoverBannerController: UIHostingController<WarrenFailoverBannerView>?
    private var failoverCancellable: AnyCancellable?
    private var failoverHideTask: Task<Void, Never>?
    private var lastShownFailoverDate: Date?

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

    private let mapViewController = MapViewController()

    override var preferredStatusBarStyle: UIStatusBarStyle {
        .lightContent
    }

    var preferredHeaderBarPresentation: HeaderBarPresentation {
        switch interactor.deviceState {
        case .loggedIn, .revoked:
            return HeaderBarPresentation(
                style: tunnelState.isSecured ? .secured : .unsecured,
                showsDivider: false
            )
        case .loggedOut:
            return HeaderBarPresentation(style: .default, showsDivider: true)
        }
    }

    var prefersHeaderBarHidden: Bool {
        false
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
        }

        interactor.didUpdateTunnelStatus = { [weak self] tunnelStatus in
            self?.connectionViewViewModel.update(tunnelStatus: tunnelStatus)
            self?.setTunnelState(tunnelStatus.state, animated: true)
            self?.indicatorsViewViewModel.tunnelState = tunnelStatus.state
            self?.indicatorsViewViewModel.observedState = tunnelStatus.observedState
            self?.view.setNeedsLayout()
        }

        interactor.didGetOutgoingAddress = { [weak self] outgoingConnectionInfo in
            self?.connectionViewViewModel.outgoingConnectionInfo = outgoingConnectionInfo
        }

        interactor.didUpdateTunnelSettings = { [weak self] tunnelSettings in
            self?.indicatorsViewViewModel.tunnelSettings = tunnelSettings
            self?.connectionViewViewModel.relayConstraints = tunnelSettings.relayConstraints
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
            }
        }

        addMapController()
        addActivityIndicator()
        addConnectionView()
        updateMap(animated: false)
        subscribeToFailoverEvents()
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

        updateMap(animated: animated)
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

    private func addConnectionView() {
        let connectionController = UIHostingController(rootView: connectionView)
        self.connectionController = connectionController

        let connectionViewProxy = connectionController.view!
        connectionViewProxy.backgroundColor = .clear

        addChild(connectionController)
        connectionController.didMove(toParent: self)
        view.addConstrainedSubviews([activityIndicator, connectionViewProxy]) {
            connectionViewProxy.pinEdgesToSuperview(.all())
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
}
