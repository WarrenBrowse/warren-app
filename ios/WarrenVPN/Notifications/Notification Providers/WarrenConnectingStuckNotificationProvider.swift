//
//  WarrenConnectingStuckNotificationProvider.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import Foundation
import UIKit
import WarrenRustRuntime
import WarrenTypes

/// "TROUBLE CONNECTING?", after a connect attempt has been running long enough
/// that the user is entitled to wonder.
///
/// iOS had no such banner. A stalled connect spun with no hint and no way out:
/// desktop has `useConnectingStuck` and Android
/// `ConnectingStuckNotificationUseCase`, both on the same 45 second window,
/// both offering the forum as the way to report it.
final class WarrenConnectingStuckNotificationProvider: NotificationProvider,
    InAppNotificationProvider
{
    /// The window desktop and Android both use.
    static let stuckAfter: TimeInterval = 45

    /// Whether the tunnel is in the middle of an attempt to come up.
    ///
    /// Collapsed to this one bool BEFORE the timer sees it: an attempt walks
    /// through several states as it redials, and arming the timer on each of
    /// them would keep pushing the banner out of reach, so it would never show.
    /// `.disconnecting` counts only when a reconnect follows, which is a redial
    /// rather than the user leaving.
    static func isConnectingPhase(_ state: TunnelState) -> Bool {
        switch state {
        case .connecting, .reconnecting, .negotiatingEphemeralPeer, .pendingReconnect:
            return true
        case let .disconnecting(actionAfterDisconnect):
            return actionAfterDisconnect == .reconnect
        case .connected, .disconnected, .waitingForConnectivity, .error:
            return false
        }
    }

    private let stuckAfter: TimeInterval
    private var tunnelObserver: TunnelBlockObserver?
    private var timer: DispatchSourceTimer?
    private var isConnecting = false
    private var isStuck = false

    init(tunnelManager: TunnelManager, stuckAfter: TimeInterval = WarrenConnectingStuckNotificationProvider.stuckAfter) {
        self.stuckAfter = stuckAfter
        super.init()

        let tunnelObserver = TunnelBlockObserver(
            didLoadConfiguration: { [weak self] tunnelManager in
                self?.handle(tunnelManager.tunnelStatus.state)
            },
            didUpdateTunnelStatus: { [weak self] _, tunnelStatus in
                self?.handle(tunnelStatus.state)
            }
        )
        self.tunnelObserver = tunnelObserver
        tunnelManager.addObserver(tunnelObserver)
    }

    deinit {
        timer?.cancel()
    }

    override var identifier: NotificationProviderIdentifier {
        .warrenConnectingStuckNotificationProvider
    }

    /// Below the tunnel-status provider, which is `.critical` and owns the
    /// blocked and no-network cases; it returns nil during an ordinary connect,
    /// which is exactly when this one speaks.
    override var priority: NotificationPriority {
        .high
    }

    var notificationDescriptor: InAppNotificationDescriptor? {
        guard isStuck else { return nil }
        return InAppNotificationDescriptor(
            identifier: identifier,
            style: .warning,
            title: NSLocalizedString(
                "TROUBLE CONNECTING?",
                comment: "Title of the banner shown when a connect attempt has run unusually long."
            ),
            body: NSAttributedString(
                string: NSLocalizedString(
                    "This is taking longer than usual. Warren keeps retrying; if it does not "
                        + "connect, report the problem on our community forum.",
                    comment: "Body of the banner shown when a connect attempt has run unusually long."
                )
            ),
            button: InAppNotificationAction(
                image: UIImage.Buttons.rightArrow,
                handler: { [weak self] in
                    self?.openCommunityForum()
                }
            )
        )
    }

    private func handle(_ state: TunnelState) {
        let connecting = Self.isConnectingPhase(state)
        // Only the edges matter. Re-arming on every status update inside one
        // attempt is the bug the phase collapse above exists to prevent.
        guard connecting != isConnecting else { return }
        isConnecting = connecting

        timer?.cancel()
        timer = nil
        if isStuck {
            isStuck = false
            invalidate()
        }
        guard connecting else { return }

        let timer = DispatchSource.makeTimerSource(queue: .main)
        timer.schedule(deadline: .now() + stuckAfter)
        timer.setEventHandler { [weak self] in
            guard let self else { return }
            isStuck = true
            invalidate()
        }
        timer.resume()
        self.timer = timer
    }

    private func openCommunityForum() {
        UIApplication.shared.open(
            URL(string: WarrenProductAnchors.current.forumPublicURL)!,
            options: [:],
            completionHandler: nil
        )
    }
}
